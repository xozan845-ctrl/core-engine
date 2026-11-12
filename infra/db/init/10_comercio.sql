-- Operación de comercio (ADR-66 a ADR-72): libro único para los recursos que el
-- frontend administra por workspace y que antes vivían en localStorage.
--
-- Se usa un payload JSONB porque cada recurso tiene un contrato propio y el
-- frontend calcula sus derivados (KPIs, filtros y saldos) en cliente. Las
-- columnas de control son las que permiten aislar por empresa o por vendedor y
-- aplicar soft delete (R-DB-5) sin borrar historia.
CREATE TABLE IF NOT EXISTS finance.workspace_registros (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id      uuid,
  propietario_id uuid,
  recurso        text NOT NULL,
  datos          jsonb NOT NULL DEFAULT '{}'::jsonb,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en   timestamptz
);

-- El ámbito siempre se consulta junto con el recurso; los índices parciales
-- evitan escanear los registros eliminados (R-DB-4).
CREATE INDEX IF NOT EXISTS idx_workspace_registros_tenant
  ON finance.workspace_registros (recurso, tenant_id)
  WHERE eliminado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_workspace_registros_propietario
  ON finance.workspace_registros (recurso, propietario_id)
  WHERE eliminado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_workspace_registros_datos
  ON finance.workspace_registros USING gin (datos jsonb_path_ops);
