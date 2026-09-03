-- Fase 4: Data Quality — Auditoría de Eventos Inválidos

-- Tabla de eventos que no pasaron el Data Contract.
-- Sirve como "Dead Letter Analítica": permite diagnosticar qué está enviando el frontend
-- o los microservicios fuente de forma incorrecta.
CREATE TABLE IF NOT EXISTS intelligence.invalid_events (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id      TEXT,                          -- ID del evento original (puede ser null si llega corrupto)
    event_tipo    TEXT,                          -- tipo del evento (ej: 'venta.geolocalizada')
    errores       TEXT[] NOT NULL,               -- Array de errores de validación
    payload       JSONB NOT NULL,                -- Payload completo para diagnóstico
    registrado_en TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Índice para buscar por tipo de evento y ventana de tiempo (diagnóstico operacional)
CREATE INDEX IF NOT EXISTS idx_invalid_events_tipo
    ON intelligence.invalid_events (event_tipo, registrado_en DESC);

-- Comentario: No necesita soft-delete. Estos registros son de auditoría y pueden
-- purgarse periódicamente por política de retención (ej: DELETE WHERE registrado_en < NOW() - INTERVAL '30 days').
