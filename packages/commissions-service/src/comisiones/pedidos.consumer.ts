import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  RabbitService,
  OutboxService,
  PgService,
  COLAS,
  EVENTOS,
  EventoBus,
  OrderCompletadoData,
  PaymentProcesadoData,
  DevolucionSolicitadaData,
  Logger,
  ejecutarConContexto,
  timeoutInternoMs,
} from '@core/shared';
import { ComisionesService } from './comisiones.service';

/**
 * Consumidor de eventos (Tabla 22):
 * - payment.procesado   -> registro del pago (entidad Pagos, simulado en MVP)
 * - order.completado    -> la comision se devenga (RN-04) y se emite
 *                          comision.acreditada para la liquidacion quincenal
 * - devolucion.solicitada -> compensacion de comision (RN-06)
 * Idempotencia por event_id y por order_id (doc 5.2).
 */
@Injectable()
export class PedidosConsumer implements OnModuleInit {
  private readonly ordersUrl = process.env.ORDERS_SERVICE_URL ?? 'http://orders-service:3004';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly logger = Logger.create('commissions.pedidos');

  constructor(
    private readonly rabbit: RabbitService,
    private readonly outbox: OutboxService,
    private readonly pg: PgService,
    private readonly comisiones: ComisionesService,
  ) {}

  async onModuleInit(): Promise<void> {
    await this.rabbit.declararColas([
      { ...COLAS.commissions_pedidos, nombre: COLAS.commissions_pedidos.cola },
      { ...COLAS.commissions_liquidacion, nombre: COLAS.commissions_liquidacion.cola },
    ]);
    await this.rabbit.consumir(COLAS.commissions_pedidos.cola, (e) => this.manejar(e));
    await this.rabbit.consumir(COLAS.commissions_liquidacion.cola, (e) => this.manejarLiquidacion(e));
    this.rabbit.activarReintento();
    this.logger.info({ msg: 'Consumidor de pedidos activo' });
  }

  private async manejarLiquidacion(evento: EventoBus): Promise<void> {
    if (evento.tipo !== EVENTOS.COMISION_ACREDITADA) return;
    // La liquidación quincenal consolida commissions.comisiones; este consumo
    // evita mensajes huérfanos en Rabbit y deja trazabilidad del evento.
    this.logger.info({ msg: 'Comision disponible para liquidacion', event_id: evento.event_id });
  }

  private async manejar(evento: EventoBus): Promise<void> {
    await ejecutarConContexto(
      {
        request_id: `commissions-event-${evento.event_id}`,
        servicio: 'commissions-consumer',
        usuario_id: '00000000-0000-0000-0000-000000000000',
        rol: 'super_admin',
      },
      async () => {
        switch (evento.tipo) {
          case EVENTOS.PAYMENT_PROCESADO:
            await this.pago(evento as EventoBus<PaymentProcesadoData>);
            break;
          case EVENTOS.ORDER_COMPLETADO:
            await this.ordenCompletada(evento as EventoBus<OrderCompletadoData>);
            break;
          case EVENTOS.DEVOLUCION_SOLICITADA:
            await this.devolucion(evento as EventoBus<DevolucionSolicitadaData>);
            break;
          default:
            return;
        }
      },
    );
  }

  private async pago(evento: EventoBus<PaymentProcesadoData>): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    await this.pg.transaccion(async (client) => {
      await this.comisiones.registrarPago(evento.data.order_id, evento.data.monto_cents, client);
      await this.marcarProcesadoEnTransaccion(client, evento);
    });
  }

  private async ordenCompletada(evento: EventoBus<OrderCompletadoData>): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    const orden = await this.consultarOrden(evento.data.order_id);
    if (!orden) throw new Error('Orden no disponible para acreditar comision.');

    if (!orden.items?.length) throw new Error('La orden no tiene lineas asignadas.');
    const acreditadas = await this.pg.transaccion(async (client) => {
      if (await this.yaProcesadoEnTransaccion(client, evento)) return 0;
      let total = 0;
      for (const item of orden.items ?? []) {
        if (!item.vendedor_id || !item.empresa_id || !item.tienda_id || item.participacion_porcentaje === undefined) {
          throw new Error('La linea no tiene contexto empresarial completo.');
        }
        const comision = await this.comisiones.acreditarLinea({
          orderId: evento.data.order_id,
          lineaClave: item.oferta_id,
          empresaId: item.empresa_id,
          tiendaId: item.tienda_id,
          vendedorId: item.vendedor_id,
          ventaCents: item.precio_unitario_cents * item.cantidad,
          participacion: item.participacion_porcentaje,
        }, client);
        if (!comision) continue;
        await this.outbox.insertarEnTransaccion(client, EVENTOS.COMISION_ACREDITADA, {
          order_id: evento.data.order_id,
          monto_cents: comision.comision_cents,
          monto_vendedor_cents: comision.monto_vendedor_cents,
          monto_empresario_cents: comision.monto_empresario_cents,
          vendedor_id: item.vendedor_id,
          empresa_id: item.empresa_id,
          tienda_id: item.tienda_id,
          participacion_porcentaje: item.participacion_porcentaje,
        });
        total += 1;
        this.logger.info({
          msg: 'Comision acreditada',
          order_id: evento.data.order_id,
          comision_cents: comision.comision_cents,
        });
      }
      await this.marcarProcesadoEnTransaccion(client, evento);
      return total;
    });
    this.logger.info({ msg: 'Comisiones procesadas', order_id: evento.data.order_id, lineas: acreditadas });
  }

  private async devolucion(evento: EventoBus<DevolucionSolicitadaData>): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    // RN-06: se compensa en la siguiente liquidacion, en una sola transacción
    // con la marca de idempotencia.
    await this.pg.transaccion(async (client) => {
      await this.comisiones.compensar(evento.data.order_id, evento.data.motivo, client);
      await this.marcarProcesadoEnTransaccion(client, evento);
    });
  }

  private async consultarOrden(orderId: string): Promise<
    { total_cents: number; items?: { oferta_id: string; vendedor_id?: string; empresa_id?: string; tienda_id?: string; participacion_porcentaje?: number; precio_unitario_cents: number; cantidad: number }[] } | null
  > {
    try {
      const res = await fetch(`${this.ordersUrl}/internal/orders/${encodeURIComponent(orderId)}`, {
        headers: { 'x-internal-key': this.claveInterna },
        signal: AbortSignal.timeout(timeoutInternoMs()),
      });
      if (!res.ok) return null;
      return (await res.json()) as { total_cents: number; items?: { oferta_id: string; vendedor_id?: string; empresa_id?: string; tienda_id?: string; participacion_porcentaje?: number; precio_unitario_cents: number; cantidad: number }[] };
    } catch {
      return null;
    }
  }

  private async yaProcesado(evento: EventoBus): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM commissions.eventos_procesados WHERE event_id = $1`,
      [evento.event_id],
    );
    return !!fila;
  }

  private async yaProcesadoEnTransaccion(
    client: import('pg').PoolClient,
    evento: EventoBus,
  ): Promise<boolean> {
    const fila = await client.query(
      `SELECT 1 FROM commissions.eventos_procesados WHERE event_id = $1`,
      [evento.event_id],
    );
    return (fila.rowCount ?? 0) > 0;
  }

  private async marcarProcesadoEnTransaccion(
    client: import('pg').PoolClient,
    evento: EventoBus,
  ): Promise<void> {
    await client.query(
      `INSERT INTO commissions.eventos_procesados (event_id, tipo, procesado_en)
       VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING`,
      [evento.event_id, evento.tipo],
    );
  }
}
