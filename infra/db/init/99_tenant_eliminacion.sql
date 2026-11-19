-- Solicitudes de eliminación de tenant con ventana de retención.
--
-- La purga la ejecuta identity-service de forma explícita cuando vence
-- `retencion_hasta`; otros servicios conservan sus propias referencias y
-- deberán sumarse a la orquestación de borrado (ADR-0035).
BEGIN;

CREATE TABLE IF NOT EXISTS identity.solicitudes_eliminacion_tenant (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  estado          text NOT NULL DEFAULT 'pendiente'
                  CHECK (estado IN ('pendiente', 'cancelada', 'purgada')),
  motivo          text NOT NULL DEFAULT '',
  retencion_hasta date NOT NULL,
  solicitado_por  uuid REFERENCES identity.usuarios(id) ON DELETE SET NULL,
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW(),
  purgado_en      timestamptz
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_solicitud_eliminacion_pendiente
  ON identity.solicitudes_eliminacion_tenant (empresa_id)
  WHERE estado = 'pendiente';
CREATE INDEX IF NOT EXISTS idx_solicitud_eliminacion_empresa
  ON identity.solicitudes_eliminacion_tenant (empresa_id, creado_en DESC);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE
      ON identity.solicitudes_eliminacion_tenant TO core_engine_app;
  END IF;
END $$;

COMMIT;
