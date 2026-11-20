-- Webhooks salientes por tenant (ADR-0045).
--
-- Cada suscripción declara una URL HTTPS, los eventos a los que se suscribe
-- (vacío = todos) y un secreto para firmar el cuerpo con HMAC-SHA256. Las
-- entregas se registran para reintentos con backoff y trazabilidad.

BEGIN;

CREATE TABLE IF NOT EXISTS notifications.webhooks (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id  uuid NOT NULL,
  url         text NOT NULL CHECK (char_length(url) BETWEEN 8 AND 500),
  descripcion text NOT NULL DEFAULT '' CHECK (char_length(descripcion) <= 200),
  eventos     text[] NOT NULL DEFAULT '{}',
  secreto     text NOT NULL CHECK (char_length(secreto) BETWEEN 32 AND 128),
  activo      boolean NOT NULL DEFAULT true,
  creado_por  uuid,
  creado_en   timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_webhooks_empresa_activo
  ON notifications.webhooks (empresa_id, activo);

CREATE TABLE IF NOT EXISTS notifications.webhook_entregas (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  webhook_id      uuid NOT NULL REFERENCES notifications.webhooks (id) ON DELETE CASCADE,
  event_id        uuid NOT NULL,
  tipo            text NOT NULL,
  payload         jsonb NOT NULL,
  estado          text NOT NULL DEFAULT 'pendiente'
                    CHECK (estado IN ('pendiente', 'entregado', 'fallido')),
  intentos        integer NOT NULL DEFAULT 0 CHECK (intentos >= 0),
  proximo_intento timestamptz,
  ultimo_error    text NOT NULL DEFAULT '',
  respuesta_status integer,
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  entregado_en    timestamptz,
  UNIQUE (webhook_id, event_id)
);

CREATE INDEX IF NOT EXISTS idx_webhook_entregas_pendientes
  ON notifications.webhook_entregas (estado, proximo_intento)
  WHERE estado = 'fallido';

CREATE INDEX IF NOT EXISTS idx_webhook_entregas_webhook
  ON notifications.webhook_entregas (webhook_id, creado_en DESC);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON notifications.webhooks TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON notifications.webhook_entregas TO core_engine_app;
  END IF;
END $$;

COMMIT;
