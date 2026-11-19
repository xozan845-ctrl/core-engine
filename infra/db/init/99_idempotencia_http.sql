-- Idempotencia HTTP de orders-service.
--
-- Guarda la respuesta 2xx de una mutación identificada por `Idempotency-Key`
-- para reenviarla en reintentos sin repetir el efecto de negocio.
BEGIN;

CREATE TABLE IF NOT EXISTS orders.idempotencia_http (
  actor_id      text NOT NULL,
  clave         text NOT NULL,
  metodo        text NOT NULL,
  ruta          text NOT NULL,
  request_hash  text NOT NULL,
  estado        text NOT NULL DEFAULT 'en_proceso'
                CHECK (estado IN ('en_proceso', 'completada')),
  status        integer,
  content_type  text,
  respuesta     bytea,
  creado_en     timestamptz NOT NULL DEFAULT NOW(),
  completado_en timestamptz,
  PRIMARY KEY (actor_id, clave)
);

CREATE INDEX IF NOT EXISTS idx_idempotencia_http_completado
  ON orders.idempotencia_http (completado_en)
  WHERE estado = 'completada';
CREATE INDEX IF NOT EXISTS idx_idempotencia_http_creado
  ON orders.idempotencia_http (creado_en)
  WHERE estado = 'en_proceso';

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON orders.idempotencia_http TO core_engine_app;
  END IF;
END $$;

COMMIT;
