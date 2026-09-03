import { Injectable } from '@nestjs/common';
import { InteligenciaRepository } from '../repositories/inteligencia.repository';
import { PgService, VentaGeolocalizadaData, Logger } from '@core/shared';

/**
 * AgregadorService: actualiza de forma atomica las vistas materializadas del
 * schema intelligence al procesar cada evento de venta o cambio de estado.
 *
 * Patron: INSERT ... ON CONFLICT DO UPDATE (upsert atomico) para garantizar
 * consistencia sin transacciones distribuidas (data-integrity.md regla 3).
 */
@Injectable()
export class AgregadorService {
  private readonly logger = Logger.create('intelligence.agregador');

  constructor(
    private readonly repo: InteligenciaRepository,
    private readonly pg: PgService,
  ) {}

  // ── Procesamiento de venta geolocalizada ──────────────────────────────────

  async procesarVentaGeolocalizada(data: VentaGeolocalizadaData): Promise<void> {
    const { skus, lat, lng, monto_cents, order_id, vendedor_id } = data;
    const montoPorSku = Math.floor(monto_cents / skus.length);

    // 1. Insertar hechos atomicamente (ON CONFLICT = idempotencia por order_id+sku)
    const hechos = skus.map((sku: string) => ({
      order_id,
      sku,
      vendedor_id,
      cantidad: 1,
      monto_cents: montoPorSku,
      lat,
      lng,
      precision: data.precision,
      velocidad: data.velocidad,
      rumbo: data.rumbo,
      tipo_actividad: data.tipo_actividad,
      resultado_visita: data.resultado_visita,
      distancia_cliente_metros: data.distancia_cliente_metros,
      rango_edad: data.rango_edad,
      genero: data.genero,
      ocurrido_en: new Date(),
    }));
    await this.repo.guardarHechosVenta(hechos);

    // 2. Punto de calor (alta resolucion, un punto por venta)
    if (lat != null && lng != null) {
      await this.pg.query(
        `INSERT INTO intelligence.puntos_calor (lat, lng, peso, vendedor_id, tipo, ocurrido_en)
         VALUES ($1, $2, $3, $4, 'venta', NOW())`,
        [lat, lng, 1, vendedor_id],
      );
    }

    // 3. Rendimiento del vendedor (upsert atomico)
    await this.pg.query(
      `INSERT INTO intelligence.rendimiento_vendedor
         (vendedor_id, total_ventas, total_monto_cents, ticket_promedio_cents, actualizado_en)
       VALUES ($1, 1, $2, $2, NOW())
       ON CONFLICT (vendedor_id) DO UPDATE SET
         total_ventas         = rendimiento_vendedor.total_ventas + 1,
         total_monto_cents    = rendimiento_vendedor.total_monto_cents + EXCLUDED.total_monto_cents,
         ticket_promedio_cents = (rendimiento_vendedor.total_monto_cents + EXCLUDED.total_monto_cents)
                                  / (rendimiento_vendedor.total_ventas + 1),
         actualizado_en       = NOW()`,
      [vendedor_id, monto_cents],
    );

    // 4. Cobertura por zona geografica (celda de ~1.1km = 0.01 grados)
    if (lat != null && lng != null) {
      const latCell = Math.floor(lat * 100) / 100;
      const lngCell = Math.floor(lng * 100) / 100;
      await this.pg.query(
        `INSERT INTO intelligence.cobertura_zona
           (lat_cell, lng_cell, total_ventas, total_monto_cents, total_vendedores, actualizado_en)
         VALUES ($1, $2, 1, $3, 1, NOW())
         ON CONFLICT (lat_cell, lng_cell) DO UPDATE SET
           total_ventas      = cobertura_zona.total_ventas + 1,
           total_monto_cents = cobertura_zona.total_monto_cents + EXCLUDED.total_monto_cents,
           actualizado_en    = NOW()`,
        [latCell, lngCell, monto_cents],
      );
    }

    // 5. Metricas por producto (upsert atomico por SKU)
    for (const sku of skus) {
      await this.pg.query(
        `INSERT INTO intelligence.metricas_producto
           (sku, total_vendido, total_monto_cents, ultima_venta_en, actualizado_en)
         VALUES ($1, 1, $2, NOW(), NOW())
         ON CONFLICT (sku) DO UPDATE SET
           total_vendido     = metricas_producto.total_vendido + 1,
           total_monto_cents = metricas_producto.total_monto_cents + EXCLUDED.total_monto_cents,
           ultima_venta_en   = NOW(),
           actualizado_en    = NOW()`,
        [sku, montoPorSku],
      );
    }

    // 6. Tendencias temporales (agrupado por dia, sku y vendedor)
    const fecha = new Date().toISOString().slice(0, 10); // YYYY-MM-DD
    const diaSemana = new Date().getDay();
    for (const sku of skus) {
      await this.pg.query(
        `INSERT INTO intelligence.tendencias_temporales
           (fecha, sku, vendedor_id, total_ventas, total_monto_cents, dia_semana)
         VALUES ($1, $2, $3, 1, $4, $5)
         ON CONFLICT (fecha, sku, vendedor_id) DO UPDATE SET
           total_ventas      = tendencias_temporales.total_ventas + 1,
           total_monto_cents = tendencias_temporales.total_monto_cents + EXCLUDED.total_monto_cents`,
        [fecha, sku, vendedor_id, montoPorSku, diaSemana],
      );
    }

    this.logger.info({
      msg: 'Venta geolocalizada agregada',
      order_id,
      vendedor_id,
      skus_count: skus.length,
      monto_cents,
      geo: lat != null ? { lat, lng } : null,
    });
  }

  // ── Procesamiento de orden completada (enriquece rendimiento) ─────────────

  async procesarOrdenCompletada(data: {
    order_id: string;
    vendedor_id: string;
    entregado_en?: string;
  }): Promise<void> {
    await this.pg.query(
      `UPDATE intelligence.rendimiento_vendedor
       SET pedidos_entregados = pedidos_entregados + 1,
           tasa_efectividad   = CASE
             WHEN (pedidos_entregados + pedidos_fallidos + 1) > 0
             THEN ROUND(((pedidos_entregados + 1)::numeric /
                         (pedidos_entregados + pedidos_fallidos + 1)) * 100, 2)
             ELSE 0 END,
           actualizado_en     = NOW()
       WHERE vendedor_id = $1`,
      [data.vendedor_id],
    );
    this.logger.info({ msg: 'Orden completada registrada', order_id: data.order_id });
  }

  // ── Procesamiento de actualizacion de stock ───────────────────────────────

  async procesarStockActualizado(data: {
    sku: string;
    stock_restante: number;
    nombre?: string;
  }): Promise<void> {
    await this.pg.query(
      `INSERT INTO intelligence.metricas_producto (sku, nombre, stock_actual, actualizado_en)
       VALUES ($1, $2, $3, NOW())
       ON CONFLICT (sku) DO UPDATE SET
         stock_actual   = EXCLUDED.stock_actual,
         nombre         = COALESCE(EXCLUDED.nombre, metricas_producto.nombre),
         actualizado_en = NOW()`,
      [data.sku, data.nombre ?? '', data.stock_restante],
    );
  }
}
