-- Subledger de ventas por línea para conciliar venta directa y RMA (ADR-0050).
-- La fila conserva el reparto original y las cantidades ya revertidas; no se
-- deriva una segunda vez el mismo asiento en reintentos o devoluciones parciales.
BEGIN;

ALTER TABLE commissions.compensaciones_devoluciones
  ADD COLUMN IF NOT EXISTS monto_comision_cents integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS cantidad integer NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS finance.ventas_lineas (
  order_id uuid NOT NULL,
  linea_clave text NOT NULL,
  empresa_id uuid NOT NULL,
  negocio_id uuid,
  directa boolean NOT NULL DEFAULT false,
  base_cents integer NOT NULL CHECK (base_cents >= 0),
  impuesto_cents integer NOT NULL DEFAULT 0 CHECK (impuesto_cents >= 0),
  comision_cents integer NOT NULL DEFAULT 0 CHECK (comision_cents >= 0),
  vendedor_cents integer NOT NULL DEFAULT 0 CHECK (vendedor_cents >= 0),
  empresario_cents integer NOT NULL DEFAULT 0 CHECK (empresario_cents >= 0),
  cantidad integer NOT NULL DEFAULT 1 CHECK (cantidad > 0),
  revertido_cantidad integer NOT NULL DEFAULT 0 CHECK (revertido_cantidad >= 0),
  revertido_base_cents integer NOT NULL DEFAULT 0 CHECK (revertido_base_cents >= 0),
  revertido_impuesto_cents integer NOT NULL DEFAULT 0 CHECK (revertido_impuesto_cents >= 0),
  revertido_comision_cents integer NOT NULL DEFAULT 0 CHECK (revertido_comision_cents >= 0),
  revertido_vendedor_cents integer NOT NULL DEFAULT 0 CHECK (revertido_vendedor_cents >= 0),
  revertido_empresario_cents integer NOT NULL DEFAULT 0 CHECK (revertido_empresario_cents >= 0),
  creado_en timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (order_id, linea_clave),
  CHECK (revertido_cantidad <= cantidad),
  CHECK (revertido_base_cents <= base_cents),
  CHECK (revertido_impuesto_cents <= impuesto_cents),
  CHECK (revertido_comision_cents <= comision_cents),
  CHECK (revertido_vendedor_cents <= vendedor_cents),
  CHECK (revertido_empresario_cents <= empresario_cents)
);
CREATE INDEX IF NOT EXISTS idx_ventas_lineas_empresa
  ON finance.ventas_lineas (empresa_id, order_id);

-- Acumulado de reembolsos confirmados por RMA. Permite que un evento
-- reentregado o fuera de orden no saque caja dos veces: solo se asienta el
-- delta entre el acumulado del evento y el ya contabilizado.
CREATE TABLE IF NOT EXISTS finance.devoluciones_reembolsadas (
  devolucion_id uuid PRIMARY KEY,
  order_id uuid NOT NULL,
  empresa_id uuid,
  reembolsado_cents integer NOT NULL DEFAULT 0 CHECK (reembolsado_cents >= 0),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_devoluciones_reembolsadas_order
  ON finance.devoluciones_reembolsadas (order_id);

ALTER TABLE finance.ventas_lineas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS ventas_lineas_tenant ON finance.ventas_lineas;
DROP POLICY IF EXISTS tenant_app ON finance.ventas_lineas;
CREATE POLICY ventas_lineas_tenant ON finance.ventas_lineas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin')
         OR empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin')
         OR empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE finance.devoluciones_reembolsadas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS devoluciones_reembolsadas_tenant ON finance.devoluciones_reembolsadas;
DROP POLICY IF EXISTS tenant_app ON finance.devoluciones_reembolsadas;
CREATE POLICY devoluciones_reembolsadas_tenant ON finance.devoluciones_reembolsadas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin')
         OR empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin')
         OR empresa_id::text = (auth.jwt() ->> 'tenant_id'));

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON finance.ventas_lineas TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON finance.devoluciones_reembolsadas TO core_engine_app;
  END IF;
END $$;

COMMIT;
