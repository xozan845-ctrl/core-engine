import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  RabbitService,
  PgService,
  COLAS,
  EVENTOS,
  EventoBus,
  StockUpdatedData,
  Logger,
} from '@core/shared';
import { OfertasService } from '../ofertas/ofertas.service';

/**
 * RN-02: al alcanzarse el stock 0, la oferta pasa a estado "agotada".
 * Consume stock.updated (editor: inventario) y sincroniza el stock visible
 * de las ofertas con el catalogo. Idempotencia por event_id (doc 5.2).
 */
@Injectable()
export class StockConsumer implements OnModuleInit {
  private readonly logger = Logger.create('stores.stock');

  constructor(
    private readonly rabbit: RabbitService,
    private readonly pg: PgService,
    private readonly ofertas: OfertasService,
  ) {}

  async onModuleInit(): Promise<void> {
    await this.rabbit.declararColas([
      { ...COLAS.stores_stock, nombre: COLAS.stores_stock.cola },
    ]);
    await this.rabbit.consumir(COLAS.stores_stock.cola, (e) => this.manejar(e));
    this.rabbit.activarReintento();
    this.logger.info({ msg: 'Consumidor de stock visible (RN-02) activo' });
  }

  private async manejar(evento: EventoBus): Promise<void> {
    if (evento.tipo !== EVENTOS.STOCK_UPDATED) return;
    if (await this.yaProcesado(evento)) return;
    const datos = (evento as EventoBus<StockUpdatedData>).data;
    for (const item of datos.items ?? []) {
      await this.ofertas.sincronizarStockDeOferta(item.sku, item.stock_restante);
    }
    await this.marcarProcesado(evento);
  }

  private async yaProcesado(evento: EventoBus): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM stores.eventos_procesados WHERE event_id = $1`,
      [evento.event_id],
    );
    return !!fila;
  }

  private async marcarProcesado(evento: EventoBus): Promise<void> {
    await this.pg.query(
      `INSERT INTO stores.eventos_procesados (event_id, tipo, procesado_en)
       VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING`,
      [evento.event_id, evento.tipo],
    );
  }
}