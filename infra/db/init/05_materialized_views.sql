-- Fase 2: Vistas Materializadas Concurrentes (CQRS)

-- 1. Vista Materializada: Resumen Ejecutivo (KPIs Globales)
-- Agrega todos los datos de hechos_venta. Se refresca en background.
CREATE MATERIALIZED VIEW IF NOT EXISTS intelligence.mv_resumen_ejecutivo AS
SELECT 
    vendedor_id,
    COUNT(DISTINCT order_id) as total_pedidos,
    SUM(monto_cents) as ingresos_totales_cents,
    COUNT(DISTINCT sku) as productos_distintos,
    NOW() as calculado_en
FROM intelligence.hechos_venta
GROUP BY vendedor_id;

-- Para permitir REFRESH MATERIALIZED VIEW CONCURRENTLY, necesitamos un UNIQUE INDEX
CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_resumen_ejecutivo_vendedor 
    ON intelligence.mv_resumen_ejecutivo (vendedor_id);

-- 2. Vista Materializada: Tendencias Temporales
-- Agrupación por día y SKU de forma dinámica desde los hechos, reemplazando la tabla manual.
CREATE MATERIALIZED VIEW IF NOT EXISTS intelligence.mv_tendencias AS
SELECT 
    DATE(ocurrido_en) as fecha,
    sku,
    vendedor_id,
    COUNT(DISTINCT order_id) as total_ventas,
    SUM(monto_cents) as total_monto_cents,
    EXTRACT(DOW FROM DATE(ocurrido_en)) as dia_semana
FROM intelligence.hechos_venta
GROUP BY DATE(ocurrido_en), sku, vendedor_id;

-- Índice único compuesto para permitir refresco concurrente
CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_tendencias_unique 
    ON intelligence.mv_tendencias (fecha, sku, vendedor_id);

-- Índices de lectura para acelerar los filtros del repositorio
CREATE INDEX IF NOT EXISTS idx_mv_tendencias_fecha ON intelligence.mv_tendencias(fecha);
CREATE INDEX IF NOT EXISTS idx_mv_tendencias_sku ON intelligence.mv_tendencias(sku);
