//! Tareas de fondo: cómputo de features (5 min) y refresco de vistas
//! materializadas (30 s), equivalentes a los `@Cron` de NestJS.

use crate::db::Db;
use crate::logs;
use crate::metrics::Metrics;
use std::sync::Arc;
use std::time::{Duration, Instant};

const GAUGE_REFRESH_SECONDS: &str = "core_engine_intelligence_view_refresh_duration_seconds";

/// Features por SKU, vendedor y zona. `Promise.all` del TypeScript: las tres
/// corren aunque una falle; se registra el primer error.
pub fn iniciar_features(db: Arc<Db>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(300));
        ticker.tick().await; // @Cron: el primer ciclo es al intervalo
        loop {
            ticker.tick().await;
            let inicio = Instant::now();
            match computar_features(&db).await {
                Ok(()) => logs::info(
                    "inteligencia.features",
                    crate::campos! {
                        "msg": format!("Features computadas en {}ms", inicio.elapsed().as_millis()),
                    },
                ),
                Err(error) => logs::error(
                    "inteligencia.features",
                    crate::campos! {
                        "msg": format!("Error computando features: {error}"),
                    },
                ),
            }
        }
    })
}

/// `Promise.all` del TypeScript: las tres corren aunque una falle; se propaga
/// el primer error para el log del cron.
pub async fn computar_features(db: &Db) -> Result<(), sqlx::Error> {
    let (sku, vendedor, zona) =
        tokio::join!(features_sku(db), features_vendedor(db), features_zona(db));
    [sku, vendedor, zona]
        .into_iter()
        .find_map(|resultado| resultado.err())
        .map_or(Ok(()), Err)
}

/// Refresco concurrente de las dos vistas materializadas.
pub fn iniciar_vistas(db: Arc<Db>, metrics: Arc<Metrics>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(30));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let inicio = Instant::now();
            match refrescar_vistas(&db).await {
                Ok(()) => {
                    let ms = inicio.elapsed().as_millis();
                    metrics.registrar_gauge(GAUGE_REFRESH_SECONDS, ms as f64 / 1000.0);
                    logs::info(
                        "inteligencia.vistas",
                        crate::campos! {
                            "msg": format!("Refresco concurrente completado en {ms}ms"),
                        },
                    );
                }
                Err(error) => logs::error(
                    "inteligencia.vistas",
                    crate::campos! {
                        "msg": format!("Error refrescando vistas materializadas: {error}"),
                    },
                ),
            }
        }
    })
}

pub async fn refrescar_vistas(db: &Db) -> Result<(), sqlx::Error> {
    sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY intelligence.mv_resumen_ejecutivo")
        .execute(db.pool())
        .await?;
    sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY intelligence.mv_tendencias")
        .execute(db.pool())
        .await?;
    Ok(())
}

async fn features_sku(db: &Db) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
         SELECT 'sku', sku, 'ventas_avg_7d', ROUND(AVG(total_diario)::NUMERIC, 4), NOW()
         FROM (
           SELECT sku, DATE(ocurrido_en) AS dia, COUNT(*) AS total_diario
           FROM intelligence.hechos_venta
           WHERE ocurrido_en >= NOW() - INTERVAL '7 days'
           GROUP BY sku, DATE(ocurrido_en)
         ) diarios
         GROUP BY sku
         ON CONFLICT (entity_type, entity_id, feature_name)
         DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()",
    )
    .execute(db.pool())
    .await?;

    sqlx::query(
        "INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
         SELECT 'sku', sku, 'ventas_std_7d',
                ROUND(COALESCE(STDDEV_POP(total_diario), 0)::NUMERIC, 4), NOW()
         FROM (
           SELECT sku, DATE(ocurrido_en) AS dia, COUNT(*) AS total_diario
           FROM intelligence.hechos_venta
           WHERE ocurrido_en >= NOW() - INTERVAL '7 days'
           GROUP BY sku, DATE(ocurrido_en)
         ) diarios
         GROUP BY sku
         ON CONFLICT (entity_type, entity_id, feature_name)
         DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()",
    )
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn features_vendedor(db: &Db) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
         SELECT 'vendedor', vendedor_id::TEXT, 'tasa_efectividad',
                ROUND(COALESCE(tasa_efectividad, 0)::NUMERIC, 4), NOW()
         FROM intelligence.rendimiento_vendedor
         ON CONFLICT (entity_type, entity_id, feature_name)
         DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()",
    )
    .execute(db.pool())
    .await?;

    sqlx::query(
        "INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
         SELECT 'vendedor', vendedor_id::TEXT, 'churn_risk_score',
                CASE
                  WHEN COALESCE(tasa_efectividad, 0) < 0.5  THEN 1.0
                  WHEN COALESCE(tasa_efectividad, 0) < 0.75 THEN 0.5
                  ELSE 0.0
                END,
                NOW()
         FROM intelligence.rendimiento_vendedor
         ON CONFLICT (entity_type, entity_id, feature_name)
         DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()",
    )
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn features_zona(db: &Db) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO intelligence.feature_store (entity_type, entity_id, feature_name, feature_value, computed_at)
         SELECT 'zona',
                ROUND(lat::NUMERIC, 2)::TEXT || ',' || ROUND(lng::NUMERIC, 2)::TEXT,
                'densidad_30d',
                COUNT(*)::NUMERIC,
                NOW()
         FROM intelligence.hechos_venta
         WHERE ocurrido_en >= NOW() - INTERVAL '30 days'
           AND lat IS NOT NULL AND lng IS NOT NULL
         GROUP BY ROUND(lat::NUMERIC, 2), ROUND(lng::NUMERIC, 2)
         ON CONFLICT (entity_type, entity_id, feature_name)
         DO UPDATE SET feature_value = EXCLUDED.feature_value, computed_at = NOW()",
    )
    .execute(db.pool())
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_usar_el_mismo_nombre_de_gauge() {
        assert_eq!(
            GAUGE_REFRESH_SECONDS,
            "core_engine_intelligence_view_refresh_duration_seconds"
        );
    }
}
