import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  RabbitService,
  PgService,
  EVENTOS,
  EventoBus,
  VentaGeolocalizadaData,
  ComisionAcreditadaData,
  StockUpdatedData,
  Logger,
} from '@core/shared';
import { AgregadorService } from '../services/agregador.service';
import { DataQualityService } from '../services/data-quality.service';

/** Colas dedicadas del servicio de inteligencia. */
const COLA_GEO    = 'intelligence.venta_geo';
const COLA_ORDERS = 'intelligence.order_completado';
const COLA_STOCK  = 'intelligence.stock_updated';

/**
 * Consumidor multi-evento para inteligencia de mercado.
 * Escucha 3 topics del bus de eventos y enruta cada uno al agregador:
 *   - venta.geolocalizada   → procesarVentaGeolocalizada (datos GPS + demografía)
 *   - order.completado      → procesarOrdenCompletada (enriquece tasa de efectividad)
 *   - stock.updated         → procesarStockActualizado (mantiene stock_actual sincronizado)
 *
 * Idempotencia garantizada via intelligence.eventos_procesados (event_id PK).
 */
@Injectable()
export class VentasConsumer implements OnModuleInit {
  private readonly logger = Logger.create('intelligence.consumer');

  constructor(
    private readonly rabbit: RabbitService,
    private readonly pg: PgService,
    private readonly agregador: AgregadorService,
    private readonly dataQuality: DataQualityService,
  ) {}

  async onModuleInit(): Promise<void> {
    // Declarar las 3 colas de inteligencia
    await this.rabbit.declararColas([
      {
        nombre: COLA_GEO,
        cola: COLA_GEO,
        routingKeys: [EVENTOS.VENTA_GEOLOCALIZADA],
      },
      {
        nombre: COLA_ORDERS,
        cola: COLA_ORDERS,
        // comision.acreditada: tiene vendedor_id, se emite tras order.completado
        routingKeys: [EVENTOS.COMISION_ACREDITADA],
      },
      {
        nombre: COLA_STOCK,
        cola: COLA_STOCK,
        routingKeys: [EVENTOS.STOCK_UPDATED],
      },
    ]);

    // Suscribir handlers independientes por cola
    await this.rabbit.consumir(COLA_GEO,    (e) => this.manejarVentaGeo(e));
    await this.rabbit.consumir(COLA_ORDERS, (e) => this.manejarOrdenCompletada(e));
    await this.rabbit.consumir(COLA_STOCK,  (e) => this.manejarStockActualizado(e));

    this.rabbit.activarReintento();
    this.logger.info({ msg: 'Consumer de inteligencia de mercado activo (3 colas)' });
  }

  // ── Handlers privados ─────────────────────────────────────────────────────

  private async manejarVentaGeo(evento: EventoBus): Promise<void> {
    if (evento.tipo !== EVENTOS.VENTA_GEOLOCALIZADA) return;
    if (await this.yaProcesado(evento.event_id)) return;

    const data = (evento as EventoBus<VentaGeolocalizadaData>).data;
    
    // Validación de Data Contract
    const qualityResult = this.dataQuality.validarVentaGeolocalizada(data);
    if (!qualityResult.isValid) {
      this.logger.error({ msg: 'Evento venta.geolocalizada rechazado por Data Contract', event_id: evento.event_id, errors: qualityResult.errors });
      // Persistir en tabla de auditoría (fire-and-forget)
      void this.dataQuality.registrarEventoInvalido(
        evento.event_id,
        evento.tipo,
        qualityResult.errors,
        data,
      );
      await this.marcarProcesado(evento.event_id, evento.tipo);
      return;
    }

    await this.agregador.procesarVentaGeolocalizada(data);
    await this.marcarProcesado(evento.event_id, evento.tipo);
    this.logger.info({ msg: 'venta.geolocalizada procesada', event_id: evento.event_id, qualityTier: qualityResult.qualityTier });
  }

  private async manejarOrdenCompletada(evento: EventoBus): Promise<void> {
    if (evento.tipo !== EVENTOS.COMISION_ACREDITADA) return;
    if (await this.yaProcesado(evento.event_id)) return;

    const data = (evento as EventoBus<ComisionAcreditadaData>).data;
    // comision.acreditada tiene vendedor_id — usamos esto para enriquecer rendimiento
    await this.agregador.procesarOrdenCompletada({
      order_id: data.order_id,
      vendedor_id: data.vendedor_id,
    });
    await this.marcarProcesado(evento.event_id, evento.tipo);
    this.logger.info({ msg: 'comision.acreditada procesada', event_id: evento.event_id });
  }

  private async manejarStockActualizado(evento: EventoBus): Promise<void> {
    if (evento.tipo !== EVENTOS.STOCK_UPDATED) return;
    if (await this.yaProcesado(evento.event_id)) return;

    const data = (evento as EventoBus<StockUpdatedData>).data;
    // StockUpdatedData.items es un array de { sku, stock_restante }
    for (const item of data.items) {
      await this.agregador.procesarStockActualizado({
        sku: item.sku,
        stock_restante: item.stock_restante,
      });
    }
    await this.marcarProcesado(evento.event_id, evento.tipo);
    this.logger.info({ msg: 'stock.updated procesado', event_id: evento.event_id });
  }

  // ── Idempotencia ──────────────────────────────────────────────────────────

  private async yaProcesado(eventId: string): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM intelligence.eventos_procesados WHERE event_id = $1`,
      [eventId],
    );
    if (fila) {
      this.logger.warn({ msg: 'Evento duplicado ignorado', event_id: eventId });
    }
    return !!fila;
  }

  private async marcarProcesado(eventId: string, tipo: string): Promise<void> {
    await this.pg.query(
      `INSERT INTO intelligence.eventos_procesados (event_id, tipo, procesado_en)
       VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING`,
      [eventId, tipo],
    );
  }
}


