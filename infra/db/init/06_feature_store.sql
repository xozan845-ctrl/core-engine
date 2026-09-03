-- Fase 3: Feature Store para Analytics Predictivo

-- 1. Tabla de Features Pre-computadas
-- Almacena variables derivadas (features) por entidad para alimentar modelos estadísticos.
-- Cada fila es un snapshot de una feature en el tiempo.
CREATE TABLE IF NOT EXISTS intelligence.feature_store (
    feature_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type     TEXT NOT NULL,   -- 'sku', 'vendedor', 'zona'
    entity_id       TEXT NOT NULL,
    feature_name    TEXT NOT NULL,
    feature_value   NUMERIC NOT NULL,
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Soft-delete no aplica aquí: features antiguas se sobreescriben con upsert
    UNIQUE (entity_type, entity_id, feature_name)
);

-- Índices de lectura para consultas por entidad y por feature
CREATE INDEX IF NOT EXISTS idx_feature_store_entity
    ON intelligence.feature_store (entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_feature_store_feature_name
    ON intelligence.feature_store (feature_name);
CREATE INDEX IF NOT EXISTS idx_feature_store_computed_at
    ON intelligence.feature_store (computed_at DESC);

-- 2. Vista para anomalías: ventas con Z-score > 2.5 en los últimos 30 días
-- El optimizador de Postgres puede usar índices subyacentes de hechos_venta
CREATE OR REPLACE VIEW intelligence.vw_anomalias_recientes AS
WITH stats AS (
    SELECT
        sku,
        AVG(monto_cents)        AS media_monto,
        STDDEV_POP(monto_cents) AS std_monto,
        COUNT(*)                AS total_muestras
    FROM intelligence.hechos_venta
    WHERE ocurrido_en >= NOW() - INTERVAL '30 days'
    GROUP BY sku
),
scored AS (
    SELECT
        h.order_id,
        h.sku,
        h.vendedor_id,
        h.monto_cents,
        h.lat,
        h.lng,
        h.ocurrido_en,
        s.media_monto,
        s.std_monto,
        CASE
            WHEN s.std_monto > 0
            THEN ABS(h.monto_cents - s.media_monto) / s.std_monto
            ELSE 0
        END AS z_score
    FROM intelligence.hechos_venta h
    JOIN stats s ON s.sku = h.sku
    WHERE h.ocurrido_en >= NOW() - INTERVAL '30 days'
      AND s.total_muestras >= 5  -- Mínimo de muestras para que el Z-score sea significativo
)
SELECT *
FROM scored
WHERE z_score > 2.5
ORDER BY z_score DESC;
