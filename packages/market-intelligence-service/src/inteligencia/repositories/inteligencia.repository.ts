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
    let query = `SELECT ST_Y(geom) as lat, ST_X(geom) as lng, peso, tipo FROM intelligence.puntos_calor WHERE deleted_at IS NULL AND geom IS NOT NULL`;
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
    
    // Filtro por radio espacial (PostGIS nativo)
    if (filtros.centro_lat != null && filtros.centro_lng != null && filtros.radio_metros != null) {
      query += ` AND ST_DWithin(geom, ST_SetSRID(ST_MakePoint($${paramCount}, $${paramCount+1}), 4326)::geography, $${paramCount+2})`;
      params.push(filtros.centro_lng, filtros.centro_lat, filtros.radio_metros);
      paramCount += 3;
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
    let query = `SELECT lat_cell, lng_cell, total_ventas, total_monto_cents, total_vendedores, actualizado_en, ST_AsGeoJSON(bounding_box) as geojson FROM intelligence.cobertura_zona WHERE 1=1`;
    const params: any[] = [];
    let paramCount = 1;

    // Podríamos usar ST_MakeEnvelope para un bounding box real
    if (filtros.lat_min != null && filtros.lat_max != null && filtros.lng_min != null && filtros.lng_max != null) {
      query += ` AND lat_cell BETWEEN $${paramCount} AND $${paramCount+1}`;
      query += ` AND lng_cell BETWEEN $${paramCount+2} AND $${paramCount+3}`;
      params.push(filtros.lat_min, filtros.lat_max, filtros.lng_min, filtros.lng_max);
      paramCount += 4;
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
        total_pedidos,
        ingresos_totales_cents,
        productos_distintos,
        calculado_en
      FROM intelligence.mv_resumen_ejecutivo
      WHERE 1=1
    `;
    const params: any[] = [];
    
    if (filtros.vendedor_id) {
      query += ` AND vendedor_id = $1`;
      params.push(filtros.vendedor_id);
    }
    
    const res = await this.pg.query<any>(query, params);
    
    // Si no hay filtro por vendedor, deberíamos agregar los resultados, 
    // pero para mantener compatibilidad con el endpoint actual, agregamos todos si no hay filtro
    if (!filtros.vendedor_id) {
      const globalQuery = `
        SELECT 
          SUM(total_pedidos) as total_pedidos,
          SUM(ingresos_totales_cents) as ingresos_totales_cents,
          COUNT(DISTINCT vendedor_id) as vendedores_activos,
          MAX(calculado_en) as calculado_en
        FROM intelligence.mv_resumen_ejecutivo
      `;
      const globalRes = await this.pg.query<any>(globalQuery, []);
      return globalRes[0] || null;
    }
    
    return res[0] || null;
  }

  // ── Tendencias temporales ─────────────────────────────────────────────────

  async obtenerTendencias(filtros: { fecha_inicio?: string; fecha_fin?: string; sku?: string; vendedor_id?: string }) {
    let query = `SELECT fecha, sku, vendedor_id, total_ventas, total_monto_cents, dia_semana FROM intelligence.mv_tendencias WHERE 1=1`;
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

  // ── Predictive Analytics ──────────────────────────────────────────────────

  /**
   * Forecast de demanda para un SKU usando la media móvil ponderada 7d del feature store.
   * Proyecta `dias` días hacia adelante multiplicando el promedio diario.
   */
  async obtenerForecastDemanda(sku: string, dias: number): Promise<any> {
    const features = await this.pg.query<any>(
      `SELECT feature_name, feature_value, computed_at
       FROM intelligence.feature_store
       WHERE entity_type = 'sku' AND entity_id = $1
         AND feature_name IN ('ventas_avg_7d', 'ventas_std_7d')`,
      [sku],
    );

    const avg7d = features.find((f) => f.feature_name === 'ventas_avg_7d')?.feature_value ?? 0;
    const std7d = features.find((f) => f.feature_name === 'ventas_std_7d')?.feature_value ?? 0;
    const computedAt = features[0]?.computed_at ?? null;

    // Proyección simple: media diaria × días solicitados ± intervalo de confianza (1 std)
    const proyeccion = parseFloat(avg7d) * dias;
    const intervaloConfianza = parseFloat(std7d) * Math.sqrt(dias);

    return {
      sku,
      dias_proyectados: dias,
      ventas_proyectadas: Math.round(proyeccion),
      intervalo_inferior: Math.max(0, Math.round(proyeccion - intervaloConfianza)),
      intervalo_superior: Math.round(proyeccion + intervaloConfianza),
      avg_diario_base: parseFloat(avg7d),
      std_diario_base: parseFloat(std7d),
      features_computadas_en: computedAt,
      metodologia: 'media_movil_ponderada_7d',
    };
  }

  /**
   * Anomalías recientes: ventas con Z-score > 2.5 vs. el historial de 30d por SKU.
   * Lee de la vista `vw_anomalias_recientes` (definida en 06_feature_store.sql).
   */
  async obtenerAnomalias(filtros: { sku?: string; vendedor_id?: string; limite?: number }): Promise<any[]> {
    let query = `
      SELECT order_id, sku, vendedor_id, monto_cents, lat, lng, ocurrido_en,
             ROUND(media_monto::NUMERIC, 2) as media_monto,
             ROUND(std_monto::NUMERIC, 2) as std_monto,
             ROUND(z_score::NUMERIC, 3) as z_score
      FROM intelligence.vw_anomalias_recientes
      WHERE 1=1
    `;
    const params: any[] = [];
    let paramCount = 1;

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

    query += ` LIMIT $${paramCount}`;
    params.push(filtros.limite || 50);

    return this.pg.query<any>(query, params);
  }

  /**
   * Score de churn y métricas predictivas de un vendedor desde el feature store.
   */
  async obtenerScoreVendedor(vendedorId: string): Promise<any> {
    const features = await this.pg.query<any>(
      `SELECT feature_name, feature_value, computed_at
       FROM intelligence.feature_store
       WHERE entity_type = 'vendedor' AND entity_id = $1`,
      [vendedorId],
    );

    if (features.length === 0) {
      return null;
    }

    const featureMap: Record<string, number> = {};
    features.forEach((f) => {
      featureMap[f.feature_name] = parseFloat(f.feature_value);
    });

    const churnScore = featureMap['churn_risk_score'] ?? 0;
    return {
      vendedor_id: vendedorId,
      tasa_efectividad: featureMap['tasa_efectividad'] ?? null,
      churn_risk_score: churnScore,
      churn_risk_nivel: churnScore >= 1.0 ? 'ALTO' : churnScore >= 0.5 ? 'MEDIO' : 'BAJO',
      features_computadas_en: features[0]?.computed_at,
    };
  }

  /**
   * Resumen de calidad de datos del pipeline: eventos inválidos, distribución por tier.
   * Alimenta el endpoint GET /inteligencia/calidad.
   */
  async obtenerCalidadDatos(horas = 24): Promise<any> {
    const [invalidos, totalProcesados] = await Promise.all([
      // Eventos inválidos agrupados por tipo de error (últimas N horas)
      this.pg.query<any>(
        `SELECT
           event_tipo,
           unnest(errores) AS error,
           COUNT(*) AS total
         FROM intelligence.invalid_events
         WHERE registrado_en >= NOW() - ($1 || ' hours')::INTERVAL
         GROUP BY event_tipo, error
         ORDER BY total DESC
         LIMIT 50`,
        [horas],
      ),
      // Total de eventos procesados en el mismo periodo
      this.pg.query<any>(
        `SELECT COUNT(*) AS total
         FROM intelligence.eventos_procesados
         WHERE procesado_en >= NOW() - ($1 || ' hours')::INTERVAL`,
        [horas],
      ),
    ]);

    const totalInvalidos = invalidos.reduce((acc: number, r: any) => acc + parseInt(r.total, 10), 0);
    const totalOk = parseInt(totalProcesados[0]?.total ?? '0', 10);
    const tasaRechazo = totalOk + totalInvalidos > 0
      ? ((totalInvalidos / (totalOk + totalInvalidos)) * 100).toFixed(2)
      : '0.00';

    return {
      ventana_horas: horas,
      eventos_procesados: totalOk,
      eventos_invalidos: totalInvalidos,
      tasa_rechazo_pct: parseFloat(tasaRechazo),
      errores_frecuentes: invalidos,
    };
  }
}
