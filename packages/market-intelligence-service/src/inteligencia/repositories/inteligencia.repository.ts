import { Injectable } from '@nestjs/common';
import { PgService } from '@core/shared';

@Injectable()
export class InteligenciaRepository {
  constructor(private readonly pg: PgService) {}

  async guardarHechosVenta(hechos: any[], clienteId?: string): Promise<void> {
    if (hechos.length === 0) return;
    
    // Simplificado para el MVP.
    for (const hecho of hechos) {
      await this.pg.query(
        `INSERT INTO intelligence.hechos_venta 
          (order_id, sku, vendedor_id, cantidad, monto_cents, lat, lng, gps_precision_metros, 
           gps_velocidad_ms, gps_rumbo_grados, tipo_actividad, resultado_visita, 
           distancia_cliente_metros, rango_edad, genero, ocurrido_en)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
         ON CONFLICT (order_id, sku) DO NOTHING`,
        [
          hecho.order_id, hecho.sku, hecho.vendedor_id, hecho.cantidad, hecho.monto_cents,
          hecho.lat, hecho.lng, hecho.precision, hecho.velocidad, hecho.rumbo,
          hecho.tipo_actividad, hecho.resultado_visita, hecho.distancia_cliente_metros,
          hecho.rango_edad, hecho.genero, hecho.ocurrido_en || new Date()
        ]
      );
    }
  }

  async obtenerMapaCalor(filtros: any): Promise<any[]> {
    let query = `SELECT lat, lng, peso, tipo FROM intelligence.puntos_calor WHERE deleted_at IS NULL`;
    const params: any[] = [];
    let paramCount = 1;

    if (filtros.vendedor_id) {
      query += ` AND vendedor_id = $${paramCount}`;
      params.push(filtros.vendedor_id);
      paramCount++;
    }

    if (filtros.sku) {
      query += ` AND sku = $${paramCount}`;
      params.push(filtros.sku);
      paramCount++;
    }

    return this.pg.query<{ lat: number; lng: number; peso: number; tipo: string }>(query, params);
  }

  // ── Rendimiento de vendedores ─────────────────────────────────────────────

  async obtenerRendimientoVendedores(
    filtros: { vendedor_id?: string; cursor?: string; limite?: number }
  ) {
    let query = `SELECT vendedor_id, total_ventas, total_monto_cents, ticket_promedio_cents, pedidos_entregados, tasa_efectividad, actualizado_en FROM intelligence.rendimiento_vendedor WHERE 1=1`;
    const params: any[] = [];
    let paramCount = 1;

    if (filtros.vendedor_id) {
      query += ` AND vendedor_id = $${paramCount}`;
      params.push(filtros.vendedor_id);
      paramCount++;
    }

    if (filtros.cursor) {
      // Keyset pagination por total_ventas (desc) y vendedor_id
      const [ventasStr, vId] = filtros.cursor.split('_');
      if (ventasStr && vId) {
        query += ` AND (total_ventas, vendedor_id) < ($${paramCount}, $${paramCount + 1})`;
        params.push(parseInt(ventasStr, 10), vId);
        paramCount += 2;
      }
    }

    query += ` ORDER BY total_ventas DESC, vendedor_id DESC LIMIT $${paramCount}`;
    params.push(filtros.limite || 50);

    const data = await this.pg.query<any>(query, params);
    
    let nextCursor = null;
    if (data.length > 0 && data.length === (filtros.limite || 50)) {
      const last = data[data.length - 1];
      nextCursor = `${last.total_ventas}_${last.vendedor_id}`;
    }

    return { data, next_cursor: nextCursor };
  }

  // ── Cobertura geografica ──────────────────────────────────────────────────

  async obtenerCoberturaZona(filtros: { lat_min?: number; lat_max?: number; lng_min?: number; lng_max?: number; limite?: number }) {
    let query = `SELECT lat_cell, lng_cell, total_ventas, total_monto_cents, total_vendedores, actualizado_en FROM intelligence.cobertura_zona WHERE 1=1`;
    const params: any[] = [];
    let paramCount = 1;

    if (filtros.lat_min != null && filtros.lat_max != null) {
      query += ` AND lat_cell BETWEEN $${paramCount} AND $${paramCount+1}`;
      params.push(filtros.lat_min, filtros.lat_max);
      paramCount += 2;
    }

    if (filtros.lng_min != null && filtros.lng_max != null) {
      query += ` AND lng_cell BETWEEN $${paramCount} AND $${paramCount+1}`;
      params.push(filtros.lng_min, filtros.lng_max);
      paramCount += 2;
    }

    query += ` ORDER BY total_ventas DESC LIMIT $${paramCount}`;
    params.push(filtros.limite || 100);

    return this.pg.query<any>(query, params);
  }

  // ── Demanda de productos ──────────────────────────────────────────────────

  async obtenerDemandaProductos(
    filtros: { sku?: string; cursor?: string; limite?: number }
  ) {
    let query = `SELECT sku, nombre, total_vendido, total_monto_cents, stock_actual, ultima_venta_en, actualizado_en FROM intelligence.metricas_producto WHERE 1=1`;
    const params: any[] = [];
    let paramCount = 1;

    if (filtros.sku) {
      query += ` AND sku = $${paramCount}`;
      params.push(filtros.sku);
      paramCount++;
    }

    if (filtros.cursor) {
      const [vendidoStr, sku] = filtros.cursor.split('_');
      if (vendidoStr && sku) {
        query += ` AND (total_vendido, sku) < ($${paramCount}, $${paramCount + 1})`;
        params.push(parseInt(vendidoStr, 10), sku);
        paramCount += 2;
      }
    }

    query += ` ORDER BY total_vendido DESC, sku DESC LIMIT $${paramCount}`;
    params.push(filtros.limite || 50);

    const data = await this.pg.query<any>(query, params);
    
    let nextCursor = null;
    if (data.length > 0 && data.length === (filtros.limite || 50)) {
      const last = data[data.length - 1];
      nextCursor = `${last.total_vendido}_${last.sku}`;
    }

    return { data, next_cursor: nextCursor };
  }

  // ── KPIs globales (resumen ejecutivo) ─────────────────────────────────────

  async obtenerResumen(filtros: { vendedor_id?: string }) {
    let query = `
      SELECT 
        COUNT(DISTINCT order_id) as total_pedidos,
        SUM(monto_cents) as ingresos_totales_cents,
        COUNT(DISTINCT vendedor_id) as vendedores_activos,
        COUNT(DISTINCT sku) as productos_distintos
      FROM intelligence.hechos_venta
      WHERE 1=1
    `;
    const params: any[] = [];
    
    if (filtros.vendedor_id) {
      query += ` AND vendedor_id = $1`;
      params.push(filtros.vendedor_id);
    }
    
    const res = await this.pg.query<any>(query, params);
    return res[0] || null;
  }

  // ── Tendencias temporales ─────────────────────────────────────────────────

  async obtenerTendencias(filtros: { fecha_inicio?: string; fecha_fin?: string; sku?: string; vendedor_id?: string }) {
    let query = `SELECT fecha, sku, vendedor_id, total_ventas, total_monto_cents, dia_semana FROM intelligence.tendencias_temporales WHERE 1=1`;
    const params: any[] = [];
    let paramCount = 1;

    if (filtros.fecha_inicio) {
      query += ` AND fecha >= $${paramCount}`;
      params.push(filtros.fecha_inicio);
      paramCount++;
    }
    
    if (filtros.fecha_fin) {
      query += ` AND fecha <= $${paramCount}`;
      params.push(filtros.fecha_fin);
      paramCount++;
    }

    if (filtros.sku) {
      query += ` AND sku = $${paramCount}`;
      params.push(filtros.sku);
      paramCount++;
    }
    
    if (filtros.vendedor_id) {
      query += ` AND vendedor_id = $${paramCount}`;
      params.push(filtros.vendedor_id);
      paramCount++;
    }

    query += ` ORDER BY fecha ASC`;

    return this.pg.query<any>(query, params);
  }
}
