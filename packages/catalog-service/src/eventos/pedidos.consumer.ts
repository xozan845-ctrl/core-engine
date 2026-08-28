import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  RabbitService,
  OutboxService,
  COLAS,
  EVENTOS,
  EventoBus,
  OrdenData,
  DevolucionSolicitadaData,
  Logger,
} from '@core/shared';
import { ProductosService } from '../productos/productos.service';

/**
 * Consumidor de eventos (Tabla 22): el catalogo descuenta stock de forma
 * atomica al crear la orden (RN-03) y revierte stock en devoluciones (RN-06).
 * Idempotencia: clave = event_id registrada en catalog.eventos_procesados.
 */
@Injectable()
export class PedidosConsumer implements OnModuleInit {
  private readonly logger = Logger.create('catalog.pedidos');

  constructor(
    private readonly rabbit: RabbitService,
    private readonly outbox: OutboxService,
    private readonly productos: ProductosService,
  ) {}

  async onModuleInit(): Promise<void> {
    await this.rabbit.declararColas([
      { ...COLAS.catalog_pedidos, nombre: COLAS.catalog_pedidos.cola },
    ]);
    await this.rabbit.consumir(COLAS.catalog_pedidos.cola, (evento) => this.manejar(evento));
    this.rabbit.activarReintento();
    this.logger.info({ msg: 'Consumidor de pedidos activo' });
  }

  private async manejar(evento: EventoBus): Promise<void> {
    switch (evento.tipo) {
      case EVENTOS.ORDER_CREATED:
        await this.ordenCreada(evento as EventoBus<OrdenData>);
        break;
      case EVENTOS.DEVOLUCION_SOLICITADA:
        await this.devolucion(evento as EventoBus<DevolucionSolicitadaData>);
        break;
      default:
        return;
    }
  }

  private async yaProcesado(evento: EventoBus): Promise<boolean> {
    return this.productos.estaProcesado(evento.event_id);
  }

  private async marcarProcesado(evento: EventoBus): Promise<void> {
    await this.productos.registrarProcesado(evento.event_id, evento.tipo);
  }

  private async ordenCreada(evento: EventoBus<OrdenData>): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    const { order_id, items } = evento.data;
    const lineas = items.map((i) => ({ sku: i.sku, cantidad: i.cantidad }));
    const resultado = await this.productos.reservarStock(order_id, lineas);

    if (resultado.ok) {
      // emite el evento de reserva junto con la escritura (outbox, ADR-03)
      await this.outbox.insertar(EVENTOS.STOCK_RESERVADO, {
        order_id,
        items: lineas.map((l) => ({ sku: l.sku, cantidad: l.cantidad })),
      });
      // stock restante por SKU (RN-02: las ofertas del stores se marcan agotadas)
      await this.outbox.insertar(EVENTOS.STOCK_UPDATED, {
        order_id,
        tipo: 'reservado',
        items: resultado.stock_restante ?? [],
      });
    } else {
      await this.outbox.insertar(EVENTOS.STOCK_FALLIDO, {
        order_id,
        items: resultado.fallidos,
      });
    }
    await this.marcarProcesado(evento);
    this.logger.info({
      msg: resultado.ok ? 'Stock reservado atomico' : 'Stock insuficiente',
      order_id,
      fallidos: resultado.fallidos.length,
    });
  }

  private async devolucion(evento: EventoBus<DevolucionSolicitadaData>): Promise<void> {
    if (await this.yaProcesado(evento)) return;
    const items = evento.data.items;
    await this.productos.reintegrarStock(evento.data.order_id, items);
    await this.outbox.insertar(EVENTOS.STOCK_REINTEGRADO, {
      order_id: evento.data.order_id,
      items: items.map((i) => ({ sku: i.sku, cantidad: i.cantidad })),
    });
    // stock.updated tras la devolucion (RN-06 revierte stock; RN-02 actualiza ofertas)
    const stock = await this.productos.stockActualDe(items.map((i) => i.sku));
    await this.outbox.insertar(EVENTOS.STOCK_UPDATED, {
      order_id: evento.data.order_id,
      tipo: 'reintegrado',
      items: stock,
    });
    await this.marcarProcesado(evento);
    this.logger.info({ msg: 'Stock reintegrado por devolucion', order_id: evento.data.order_id });
  }
}