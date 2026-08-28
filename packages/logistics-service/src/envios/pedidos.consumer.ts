import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  RabbitService,
  OutboxService,
  PgService,
  COLAS,
  EVENTOS,
  EventoBus,
  PaymentProcesadoData,
  OrderStatusUpdatedData,
  Logger,
} from '@core/shared';
import { EnviosService } from './envios.service';

/**
 * Consumidor de eventos de pagos y estados (Tabla 22):
 * - payment.procesado    -> la bodega prepara el envio (guia de despacho)
 * - order.status.updated -> notificaciones; si Entregada emite order.completado
 *                           (las comisiones devengan); si Devuelta emite
 *                           devolucion.solicitada (Pedidos retornan stock).
 * Idempotencia por event_id (doc 5.2).
 */
@Injectable()
export class PedidosConsumer implements OnModuleInit {
  private readonly logger = Logger.create('logistics.pagos');

  constructor(
    private readonly rabbit: RabbitService,
    private readonly outbox: OutboxService,
    private readonly pg: PgService,
    private readonly envios: EnviosService,
  ) {}

  async onModuleInit(): Promise<void> {
    await this.rabbit.declararColas([
      { ...COLAS.logistics_pagos, nombre: COLAS.logistics_pagos.cola },
    ]);
    await this.rabbit.consumir(COLAS.logistics_pagos.cola, (e) => this.manejar(e));
    this.rabbit.activarReintento();
    this.logger.info({ msg: 'Consumidor de pagos/estados activo' });
  }

  private async manejar(evento: EventoBus): Promise<void> {
    switch (evento.tipo) {
      case EVENTOS.PAYMENT_PROCESADO:
        await this.pagoProcesado(evento as EventoBus<PaymentProcesadoData>);
        break;
      case EVENTOS.ORDER_STATUS_UPDATED:
        await this.estadoActualizado(evento as EventoBus<OrderStatusUpdatedData>);
        break;
      default:
        return;
    }
  }

  private async pagoProcesado(evento: EventoBus<PaymentProcesadoData>): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    const { order_id, monto_cents } = evento.data;
    const envio = await this.envios.prepararEnvio(order_id, monto_cents);
    await this.outbox.insertar(EVENTOS.SHIPMENT_STARTED, {
      order_id,
      guia: envio.guia,
      estado: 'en_preparacion',
    });
    this.notificar('email', order_id, 'La bodega alista tu pedido', {
      comprador: true,
      vendedor: true,
    });
    await this.marcarProcesado(evento);
  }

  private async estadoActualizado(evento: EventoBus<OrderStatusUpdatedData>): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    const { order_id, estado, previo_estado, motivo } = evento.data;

    this.notificar('email', order_id, `Tu pedido paso a estado "${estado}"`, {
      comprador: true,
      vendedor: true,
    });

    if (estado === 'entregada') {
      // las comisiones devengan comision por orden completada
      await this.outbox.insertar(EVENTOS.ORDER_COMPLETADO, {
        order_id,
        entregado_en: new Date().toISOString(),
      });
      this.logger.info({ msg: 'Orden completada: comision listo para devengar', order_id });
    } else if (estado === 'devuelta') {
      const orden = await this.envios.consultarOrden(order_id);
      const items = (orden?.items ?? []).map((i) => ({ sku: i.sku, cantidad: i.cantidad }));
      // RN-06: Pedidos retornan stock; Pagos reembolsan comision
      await this.outbox.insertar(EVENTOS.DEVOLUCION_SOLICITADA, {
        order_id,
        motivo: motivo ?? 'devolucion aprobada',
        items,
      });
      this.logger.info({ msg: 'Devolucion solicitada: stock y comision se revertiran', order_id });
    }

    void previo_estado;
    await this.marcarProcesado(evento);
  }

  /** RF-07: notificaciones por correo/push (simuladas con logs estructurados). */
  private notificar(canal: 'email' | 'push', orderId: string, mensaje: string, destinatarios: { comprador: boolean; vendedor: boolean }): void {
    this.logger.info({
      msg: 'notificacion_enviada',
      canal,
      order_id: orderId,
      texto: mensaje,
      destinatarios,
    });
  }

  private async yaProcesado(evento: EventoBus): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM logistics.eventos_procesados WHERE event_id = $1`,
      [evento.event_id],
    );
    return !!fila;
  }

  private async marcarProcesado(evento: EventoBus): Promise<void> {
    await this.pg.query(
      `INSERT INTO logistics.eventos_procesados (event_id, tipo, procesado_en)
       VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING`,
      [evento.event_id, evento.tipo],
    );
  }
}