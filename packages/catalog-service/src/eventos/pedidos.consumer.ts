import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  RabbitService,
  OutboxService,
  PgService,
  COLAS,
  EVENTOS,
  EventoBus,
  OrdenData,
  DevolucionSolicitadaData,
  Logger,
  ejecutarConContexto,
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
    private readonly pg: PgService,
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
    await ejecutarConContexto(
      {
        request_id: `catalog-event-${evento.event_id}`,
        servicio: 'catalog-consumer',
        usuario_id: '00000000-0000-0000-0000-000000000000',
        rol: 'super_admin',
      },
      async () => {
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
      },
    );
  }

  private async ordenCreada(evento: EventoBus<OrdenData>): Promise<void> {
    const { order_id, items } = evento.data;
    const lineas = items.map((i) => ({
      empresa_id: i.empresa_id,
      sku: i.sku,
      cantidad: i.cantidad,
    }));

    // Stock + eventos + marca de idempotencia comparten UNA transacción: si
    // falla el outbox, el descuento de stock también revierte y el reintento
    // es seguro.
    const resultado = await this.pg.transaccion(async (client) => {
      if (await this.productos.estaProcesado(evento.event_id, client)) return null;
      const reserva = await this.productos.reservarStockEnTransaccion(client, order_id, lineas);
      if (reserva.ok) {
        await this.outbox.insertarEnTransaccion(client, EVENTOS.STOCK_RESERVADO, {
          order_id,
          items: lineas.map((l) => ({ sku: l.sku, cantidad: l.cantidad })),
        });
        await this.outbox.insertarEnTransaccion(client, EVENTOS.STOCK_UPDATED, {
          order_id,
          tipo: 'reservado',
          items: reserva.stock_restante ?? [],
        });
      } else {
        await this.outbox.insertarEnTransaccion(client, EVENTOS.STOCK_FALLIDO, {
          order_id,
          items: reserva.fallidos,
        });
      }
      await this.productos.registrarProcesado(evento.event_id, evento.tipo, client);
      return reserva;
    });
    if (!resultado) return;

    this.logger.info({
      msg: resultado.ok ? 'Stock reservado atomico' : 'Stock insuficiente',
      order_id,
      fallidos: resultado.fallidos.length,
    });
  }

  private async devolucion(evento: EventoBus<DevolucionSolicitadaData>): Promise<void> {
    const items = evento.data.items;
    const reintegrado = await this.pg.transaccion(async (client) => {
      if (await this.productos.estaProcesado(evento.event_id, client)) return false;
      await this.productos.reintegrarStockEnTransaccion(
        client,
        evento.data.order_id,
        items.map((i) => ({ empresa_id: i.empresa_id, sku: i.sku, cantidad: i.cantidad })),
      );
      await this.outbox.insertarEnTransaccion(client, EVENTOS.STOCK_REINTEGRADO, {
        order_id: evento.data.order_id,
        items: items.map((i) => ({ sku: i.sku, cantidad: i.cantidad })),
      });
      // stock.updated tras la devolucion (RN-06 revierte stock; RN-02 actualiza ofertas)
      const stock = await this.productos.stockActualDe(
        items.map((i) => ({ empresa_id: i.empresa_id, sku: i.sku })),
        client,
      );
      await this.outbox.insertarEnTransaccion(client, EVENTOS.STOCK_UPDATED, {
        order_id: evento.data.order_id,
        tipo: 'reintegrado',
        items: stock,
      });
      await this.productos.registrarProcesado(evento.event_id, evento.tipo, client);
      return true;
    });
    if (!reintegrado) return;
    this.logger.info({ msg: 'Stock reintegrado por devolucion', order_id: evento.data.order_id });
  }
}
