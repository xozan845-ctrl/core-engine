import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  RabbitService,
  PgService,
  COLAS,
  EVENTOS,
  EventoBus,
  StockReservadoData,
  StockFallidoData,
  Logger,
  ejecutarConContexto,
} from '@core/shared';
import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';

/**
 * Consumidor de resultados de stock (Tabla 22):
 * - stock.reservado  -> la orden pasa a Pagada y se emite payment.procesado
 *                       (pago simulado del MVP; el reembolso real vivira en la
 *                       pasarela, hoja de ruta 6.6)
 * - stock.fallido    -> la orden se rechaza (RN-03) y queda Cancelada
 * Idempotencia por event_id (doc 5.2: deduplicar por identificador de negocio).
 */
@Injectable()
export class PedidosConsumer implements OnModuleInit {
  private readonly logger = Logger.create('orders.resultados');

  constructor(
    private readonly rabbit: RabbitService,
    private readonly pg: PgService,
    private readonly handler: CreateOrderCommandHandler,
  ) {}

  async onModuleInit(): Promise<void> {
    await this.rabbit.declararColas([
      { ...COLAS.orders_resultados, nombre: COLAS.orders_resultados.cola },
    ]);
    await this.rabbit.consumir(COLAS.orders_resultados.cola, (e) => this.manejar(e));
    this.rabbit.activarReintento();
    this.logger.info({ msg: 'Consumidor de resultados de stock activo' });
  }

  private async manejar(evento: EventoBus): Promise<void> {
    await ejecutarConContexto(
      {
        request_id: `orders-event-${evento.event_id}`,
        servicio: 'orders-consumer',
        usuario_id: '00000000-0000-0000-0000-000000000000',
        rol: 'super_admin',
      },
      async () => {
        switch (evento.tipo) {
          case EVENTOS.STOCK_RESERVADO:
            await this.stockReservado(evento as EventoBus<StockReservadoData>);
            break;
          case EVENTOS.STOCK_FALLIDO:
            await this.stockFallido(evento as EventoBus<StockFallidoData>);
            break;
          default:
            return;
        }
      },
    );
  }

  private async stockReservado(evento: EventoBus<StockReservadoData>): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    const orderId = evento.data.order_id;
    const actual = await this.pg.queryOne<{
      estado: string;
      total_cents: number;
      metodo_pago: string;
    }>(
      `SELECT estado, total_cents, metodo_pago FROM orders.orden_vista WHERE id = $1`,
      [orderId],
    );
    if (actual && actual.estado === 'creada') {
      const esCredito = actual.metodo_pago === 'credito';
      // pago.procesado viaja en la misma transacción que la transición a
      // `pagada` (no puede quedar una orden pagada sin evento de pago).
      await this.handler.transicionar(
        orderId,
        'pagada',
        esCredito ? 'pago_credito_aprobado' : 'pago_simulado_aprobado',
        'inventario',
        {
          tipo: EVENTOS.PAYMENT_PROCESADO,
          data: {
            order_id: orderId,
            monto_cents: actual.total_cents,
            estado: 'procesado',
            metodo: esCredito ? 'credito' : 'simulado',
          },
        },
      );
      this.logger.info({
        msg: esCredito ? 'Venta a credito aprobada' : 'Pago simulado procesado',
        order_id: orderId,
      });
    }
    await this.marcarProcesado(evento);
  }

  private async stockFallido(evento: EventoBus<StockFallidoData>): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    const orderId = evento.data.order_id;
    const actual = await this.pg.queryOne<{ estado: string }>(
      `SELECT estado FROM orders.orden_vista WHERE id = $1`,
      [orderId],
    );
    if (actual && actual.estado === 'creada') {
      await this.handler.transicionar(
        orderId,
        'cancelada',
        `stock_insuficiente: ${evento.data.items.map((i) => i.sku).join(', ')}`,
        'inventario',
      );
      this.logger.warn({ msg: 'Orden rechazada por stock', order_id: orderId });
    }
    await this.marcarProcesado(evento);
  }

  private async yaProcesado(evento: EventoBus): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM orders.eventos_procesados WHERE event_id = $1`,
      [evento.event_id],
    );
    return !!fila;
  }

  private async marcarProcesado(evento: EventoBus): Promise<void> {
    await this.pg.query(
      `INSERT INTO orders.eventos_procesados (event_id, tipo, procesado_en)
       VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING`,
      [evento.event_id, evento.tipo],
    );
  }
}