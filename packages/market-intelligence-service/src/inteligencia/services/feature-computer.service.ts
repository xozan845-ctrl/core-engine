import { Injectable, Logger } from '@nestjs/common';
import { Cron, CronExpression } from '@nestjs/schedule';
import { PgService } from '@core/shared';

/**
 * Computa y upserta features estadísticas en intelligence.feature_store.
 * Corre cada 5 minutos. Usa SQL puro con window functions de Postgres:
 *   - Promedio móvil 7 días (ventas_avg_7d)
 *   - Desviación estándar 7 días (ventas_std_7d) → base para detección de anomalías
 *   - Tasa de efectividad del vendedor
 *   - Densidad de zona (30 días)
 */
@Injectable()
export class FeatureComputerService {
  private readonly logger = new Logger(FeatureComputerService.name);

  constructor(private readonly pg: PgService) {}

  @Cron(CronExpression.EVERY_5_MINUTES)
  async computeFeatures(): Promise<void> {
    this.logger.debug('Iniciando cómputo de features...');
    const start = Date.now();

    try {
      await Promise.all([
        this.computeSkuFeatures(),
        this.computeVendedorFeatures(),
        this.computeZonaFeatures(),
      ]);
      this.logger.log(`Features computadas en ${Date.now() - start}ms`);
    } catch (err: any) {
      this.logger.error(`Error computando features: ${err.message}`, err.stack);
    }
  }

  // ── 1. Features por SKU ──────────────────────────────────────────────────

  private async computeSkuFeatures(): Promise<void> {
    // Promedio móvil y desviación estándar de ventas diarias en los últimos 7 días
    await this.pg.query(`
      INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
      SELECT
        'sku',
        sku,
        'ventas_avg_7d',
        ROUND(AVG(total_diario)::NUMERIC, 4),
        NOW()
      FROM (
        SELECT
          sku,
          DATE(ocurrido_en) AS dia,
          COUNT(*) AS total_diario
        FROM intelligence.hechos_venta
        WHERE ocurrido_en >= NOW() - INTERVAL '7 days'
        GROUP BY sku, DATE(ocurrido_en)
      ) diarios
      GROUP BY sku
      ON CONFLICT (entity_type, entity_id, feature_name)
      DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()
    `);

    // Desviación estándar (base para Z-score de anomalías futuras)
    await this.pg.query(`
      INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
      SELECT
        'sku',
        sku,
        'ventas_std_7d',
        ROUND(COALESCE(STDDEV_POP(total_diario), 0)::NUMERIC, 4),
        NOW()
      FROM (
        SELECT
          sku,
          DATE(ocurrido_en) AS dia,
          COUNT(*) AS total_diario
        FROM intelligence.hechos_venta
        WHERE ocurrido_en >= NOW() - INTERVAL '7 days'
        GROUP BY sku, DATE(ocurrido_en)
      ) diarios
      GROUP BY sku
      ON CONFLICT (entity_type, entity_id, feature_name)
      DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()
    `);
  }

  // ── 2. Features por Vendedor ─────────────────────────────────────────────

  private async computeVendedorFeatures(): Promise<void> {
    // Tasa de efectividad = pedidos_entregados / total_ventas (desde rendimiento_vendedor)
    await this.pg.query(`
      INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
      SELECT
        'vendedor',
        vendedor_id::TEXT,
        'tasa_efectividad',
        ROUND(COALESCE(tasa_efectividad, 0)::NUMERIC, 4),
        NOW()
      FROM intelligence.rendimiento_vendedor
      ON CONFLICT (entity_type, entity_id, feature_name)
      DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()
    `);

    // Score de riesgo de churn: si la tasa de efectividad bajó vs. la semana anterior
    // (comparamos rendimiento_vendedor.tasa_efectividad vs. promedio 14d de hechos)
    await this.pg.query(`
      INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
      SELECT
        'vendedor',
        vendedor_id::TEXT,
        'churn_risk_score',
        -- Score: 0 = sin riesgo, 1 = riesgo alto
        -- Si tasa_efectividad < 0.5 → riesgo alto; entre 0.5 y 0.75 → riesgo medio
        CASE
          WHEN COALESCE(tasa_efectividad, 0) < 0.5  THEN 1.0
          WHEN COALESCE(tasa_efectividad, 0) < 0.75 THEN 0.5
          ELSE 0.0
        END,
        NOW()
      FROM intelligence.rendimiento_vendedor
      ON CONFLICT (entity_type, entity_id, feature_name)
      DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()
    `);
  }

  // ── 3. Features por Zona (celda geográfica) ──────────────────────────────

  private async computeZonaFeatures(): Promise<void> {
    // Densidad de ventas por celda geográfica en 30 días
    await this.pg.query(`
      INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
      SELECT
        'zona',
        -- Celda de 0.01 grados (~1.1km) como entity_id
        ROUND(lat::NUMERIC, 2)::TEXT || ',' || ROUND(lng::NUMERIC, 2)::TEXT,
        'densidad_30d',
        COUNT(*)::NUMERIC,
        NOW()
      FROM intelligence.hechos_venta
      WHERE ocurrido_en >= NOW() - INTERVAL '30 days'
        AND lat IS NOT NULL AND lng IS NOT NULL
      GROUP BY ROUND(lat::NUMERIC, 2), ROUND(lng::NUMERIC, 2)
      ON CONFLICT (entity_type, entity_id, feature_name)
      DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()
    `);
  }
}
