import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  RabbitService,
  PgService,
  COLAS,
  EVENTOS,
  EventoBus,
  OrdenData,
  PaymentProcesadoData,
  ComisionAcreditadaData,
  DevolucionSolicitadaData,
  Logger,
  Money,
} from '@core/shared';
import { ContabilidadService } from '../contabilidad/contabilidad.service';
import { MetricasService } from './metricas.service';
import type { PoolClient } from 'pg';

/**
 * Consumidor de eventos (Tabla 22) que alimenta la contabilidad y el tablero:
 * - payment.procesado     -> asiento: Caja / Fondos por liquidar + metrica GMV
 * - comision.acreditada   -> asiento: realizacion del ingreso (RN-04): fondos
 *                            a Ingresos por comisiones y a Acreedores vendedores
 * - devolucion.solicitada -> asiento de ajuste con la comision revertida (RN-06)
 * Idempotencia por event_id (doc 5.2) y persistencia atomica con el outbox
 * (ADR-03): el asiento, las metricas y el evento saliente comparten transaccion.
 */
@Injectable()
export class EventsConsumer implements OnModuleInit {
  private readonly ordersUrl = process.env.ORDERS_SERVICE_URL ?? 'http://orders-service:3004';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly comisionTasa = Number(process.env.COMMISSION_RATE ?? 0.12);
  private readonly logger = Logger.create('finance.pedidos');

  constructor(
    private readonly rabbit: RabbitService,
    private readonly pg: PgService,
    private readonly contabilidad: ContabilidadService,
    private readonly metricas: MetricasService,
  ) {}

  async onModuleInit(): Promise<void> {
    await this.rabbit.declararColas([
      { ...COLAS.finance_pedidos, nombre: COLAS.finance_pedidos.cola },
    ]);
    await this.rabbit.consumir(COLAS.finance_pedidos.cola, (e) => this.manejar(e));
    this.rabbit.activarReintento();
    this.logger.info({ msg: 'Consumidor de eventos financieros activo' });
  }

  private async manejar(evento: EventoBus): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    switch (evento.tipo) {
      case EVENTOS.ORDER_CREATED:
        await this.checkout(evento as EventoBus<OrdenData>);
        break;
      case EVENTOS.PAYMENT_PROCESADO:
        await this.pago(evento as EventoBus<PaymentProcesadoData>);
        break;
      case EVENTOS.COMISION_ACREDITADA:
        await this.comisionAcreditada(evento as EventoBus<ComisionAcreditadaData>);
        break;
      case EVENTOS.DEVOLUCION_SOLICITADA:
        await this.devolucion(evento as EventoBus<DevolucionSolicitadaData>);
        break;
      default:
        return;
    }
  }

  /** Embudo (8.8): el checkout completado es la creacion de la orden (order.created). */
  private async checkout(evento: EventoBus<OrdenData>): Promise<void> {
    await this.pg.transaccion(async (client) => {
      await this.metricas.sumarMetrica(client, 'checkouts_completados', 0, 1);
      await this.marcarEnTransaccion(client, evento);
    });
    this.logger.info({ msg: 'Checkout contabilizado para el embudo', order_id: evento.data.order_id });
  }

  /** Pago de la orden: entra la caja y nace el pasivo con el vendedor. */
  private async pago(evento: EventoBus<PaymentProcesadoData>): Promise<void> {
    const { order_id, monto_cents } = evento.data;
    await this.pg.transaccion(async (client) => {
      await this.contabilidad.registrarEnTransaccion(client, {
        concepto: `Pago de la orden ${order_id}`,
        tipo: 'INGRESO',
        referencia_tipo: 'order',
        referencia_id: order_id,
        creado_por: 'sistema',
        detalles: [
          { cuenta_codigo: '1.1.1', debe_cents: monto_cents, haber_cents: 0, concepto: 'Caja' },
          { cuenta_codigo: '2.1.2', debe_cents: 0, haber_cents: monto_cents, concepto: 'Fondos por liquidar' },
        ],
      });
      await this.metricas.sumarMetrica(client, 'pedidos', 0, 1);
      await this.metricas.sumarMetrica(client, 'gmv', monto_cents, 0);
      await this.marcarEnTransaccion(client, evento);
    });
    this.logger.info({ msg: 'Asiento de pago registrado', order_id, monto_cents });
  }

  /** La orden entregada realiza el ingreso (RN-04): comision + parte vendedor. */
  private async comisionAcreditada(evento: EventoBus<ComisionAcreditadaData>): Promise<void> {
    const { order_id, monto_cents, monto_vendedor_cents, vendedor_id } = evento.data;
    const ventaCents = monto_cents + monto_vendedor_cents;
    await this.pg.transaccion(async (client) => {
      await this.contabilidad.registrarEnTransaccion(client, {
        concepto: `Realizacion de venta ${order_id}: comision (RN-04) y parte del vendedor`,
        tipo: 'AJUSTE',
        referencia_tipo: 'order',
        referencia_id: order_id,
        creado_por: 'sistema',
        detalles: [
          { cuenta_codigo: '2.1.2', debe_cents: ventaCents, haber_cents: 0, concepto: 'Fondos por liquidar' },
          { cuenta_codigo: '4.1', debe_cents: 0, haber_cents: monto_cents, concepto: 'Comision de la plataforma' },
          { cuenta_codigo: '2.1.1', debe_cents: 0, haber_cents: monto_vendedor_cents, concepto: 'Liquidacion al vendedor' },
        ],
      });
      await this.metricas.sumarMetrica(client, 'ingresos_comisiones', monto_cents, 0);
      await this.metricas.sumarMetrica(client, 'entregados', 0, 1);
      await this.metricas.incrementarVendedor(client, vendedor_id, {
        ventas: 1,
        monto_cents: ventaCents,
        comision_cents: monto_cents,
      });
      await this.marcarEnTransaccion(client, evento);
    });
    this.logger.info({ msg: 'Asiento de comision registrado', order_id, comision_cents: monto_cents });
  }

  /** Devolucion (RN-06): revierte la comision acreditada y la devuelve al acreedor. */
  private async devolucion(evento: EventoBus<DevolucionSolicitadaData>): Promise<void> {
    const orden = await this.consultarOrden(evento.data.order_id);
    if (!orden) throw new Error('Orden no disponible para el asiento de devolucion.');
    const comision = Money.desdeCentavos(orden.total_cents).comision(this.comisionTasa).centavos;

    await this.pg.transaccion(async (client) => {
      await this.contabilidad.registrarEnTransaccion(client, {
        concepto: `Devolucion de la orden ${evento.data.order_id} (RN-06): compensacion de comision`,
        tipo: 'AJUSTE',
        referencia_tipo: 'order',
        referencia_id: evento.data.order_id,
        creado_por: 'sistema',
        detalles: [
          { cuenta_codigo: '4.1', debe_cents: comision, haber_cents: 0, concepto: 'Comision revertida' },
          { cuenta_codigo: '2.1.1', debe_cents: 0, haber_cents: comision, concepto: 'Compensacion en liquidacion' },
        ],
      });
      await this.metricas.sumarMetrica(client, 'devoluciones', 0, 1);
      await this.marcarEnTransaccion(client, evento);
    });
    this.logger.info({ msg: 'Asiento de devolucion registrado', order_id: evento.data.order_id, comision_cents: comision });
  }

  private async consultarOrden(
    orderId: string,
  ): Promise<{ total_cents: number } | null> {
    try {
      const res = await fetch(`${this.ordersUrl}/internal/orders/${encodeURIComponent(orderId)}`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) return null;
      return (await res.json()) as { total_cents: number };
    } catch {
      return null;
    }
  }

  private async yaProcesado(evento: EventoBus): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM finance.eventos_procesados WHERE event_id = $1`,
      [evento.event_id],
    );
    return !!fila;
  }

  /**
   * Deduplicacion atomica (ADR-03): el marcado vive en la MISMA transaccion
   * que el asiento y las metricas; si el commit no ocurre, el reintento del
   * bus no duplica nada (doc 5.2).
   */
  private async marcarEnTransaccion(client: PoolClient, evento: EventoBus): Promise<void> {
    await client.query(
      `INSERT INTO finance.eventos_procesados (event_id, tipo, procesado_en)
       VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING`,
      [evento.event_id, evento.tipo],
    );
  }
}