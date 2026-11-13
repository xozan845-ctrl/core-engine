-- Fase P2.3 · suscripciones y ordenes recurrentes.
-- Una suscripcion referencia una oferta y crea una orden cuando vence.
BEGIN;

INSERT INTO identity.capacidades (codigo, nombre, modulo) VALUES
  ('suscripciones', 'Suscripciones y recurrencia', 'orders')
ON CONFLICT (codigo) DO NOTHING;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'suscripciones', '{}'::jsonb, true
  FROM identity.verticales v
 WHERE v.activo = true
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'suscripciones', '{}'::jsonb, true
  FROM identity.packs p
 WHERE p.activo = true
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO NOTHING;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, 'suscripciones', '{}'::jsonb, true
  FROM identity.empresas e
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

CREATE TABLE IF NOT EXISTS orders.suscripciones (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id        uuid NOT NULL,
  oferta_id         uuid NOT NULL,
  item_id           uuid,
  cantidad          integer NOT NULL DEFAULT 1 CHECK (cantidad BETWEEN 1 AND 99),
  comprador_id      uuid NOT NULL,
  cliente_parte_id  uuid,
  tipo_orden        text NOT NULL DEFAULT 'compra' CHECK (tipo_orden IN ('compra')),
  metodo_pago       text NOT NULL DEFAULT 'simulado' CHECK (metodo_pago IN ('simulado', 'credito')),
  fulfillment       text NOT NULL DEFAULT 'none'
                    CHECK (fulfillment IN ('delivery', 'pickup', 'onsite', 'digital', 'none')),
  intervalo         text NOT NULL CHECK (intervalo IN ('semanal', 'mensual', 'anual')),
  estado            text NOT NULL DEFAULT 'activa' CHECK (estado IN ('activa', 'pausada', 'cancelada')),
  proxima_ejecucion date NOT NULL,
  ultima_ejecucion  date,
  fin_en            date,
  metadata          jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  creado_por        uuid NOT NULL,
  creado_en         timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en    timestamptz NOT NULL DEFAULT NOW(),
  CHECK (fin_en IS NULL OR fin_en >= proxima_ejecucion)
);

CREATE INDEX IF NOT EXISTS idx_suscripciones_empresa_estado
  ON orders.suscripciones (empresa_id, estado, proxima_ejecucion);
CREATE INDEX IF NOT EXISTS idx_suscripciones_comprador
  ON orders.suscripciones (comprador_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_suscripciones_vencidas
  ON orders.suscripciones (proxima_ejecucion)
  WHERE estado = 'activa';

CREATE TABLE IF NOT EXISTS orders.suscripcion_ejecuciones (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  suscripcion_id  uuid NOT NULL REFERENCES orders.suscripciones(id) ON DELETE CASCADE,
  order_id        uuid,
  estado          text NOT NULL CHECK (estado IN ('creada', 'error')),
  error           text NOT NULL DEFAULT '',
  ejecutada_en    timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_suscripcion_ejecuciones_suscripcion
  ON orders.suscripcion_ejecuciones (suscripcion_id, ejecutada_en DESC);

COMMENT ON TABLE orders.suscripciones IS
  'Oferta comprada periodicamente; cada vencimiento crea una orden.';
COMMENT ON TABLE orders.suscripcion_ejecuciones IS
  'Historial de renovaciones manuales o automaticas.';

GRANT SELECT, INSERT, UPDATE, DELETE ON orders.suscripciones TO core_engine_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON orders.suscripcion_ejecuciones TO core_engine_app;

ALTER TABLE orders.suscripciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS suscripciones_admin ON orders.suscripciones;
CREATE POLICY suscripciones_admin ON orders.suscripciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS suscripciones_empresa ON orders.suscripciones;
CREATE POLICY suscripciones_empresa ON orders.suscripciones
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS suscripciones_comprador ON orders.suscripciones;
CREATE POLICY suscripciones_comprador ON orders.suscripciones
  FOR SELECT TO authenticated
  USING (auth.rol() = 'comprador' AND comprador_id = auth.uid());

ALTER TABLE orders.suscripcion_ejecuciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS suscripcion_ejecuciones_admin ON orders.suscripcion_ejecuciones;
CREATE POLICY suscripcion_ejecuciones_admin ON orders.suscripcion_ejecuciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS suscripcion_ejecuciones_empresa ON orders.suscripcion_ejecuciones;
CREATE POLICY suscripcion_ejecuciones_empresa ON orders.suscripcion_ejecuciones
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS suscripcion_ejecuciones_comprador ON orders.suscripcion_ejecuciones;
CREATE POLICY suscripcion_ejecuciones_comprador ON orders.suscripcion_ejecuciones
  FOR SELECT TO authenticated
  USING (auth.rol() = 'comprador' AND empresa_id::text IN (
    SELECT empresa_id::text FROM orders.suscripciones s
     WHERE s.comprador_id = auth.uid()
  ));

COMMIT;
