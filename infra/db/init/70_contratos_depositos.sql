-- Fase P2.2 · contratos, depositos y garantias.
-- Primitiva agnostica para alquileres, servicios y acuerdos con retencion.
BEGIN;

INSERT INTO identity.capacidades (codigo, nombre, modulo) VALUES
  ('contratos', 'Contratos y depositos', 'orders')
ON CONFLICT (codigo) DO NOTHING;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'contratos', '{}'::jsonb, true
  FROM identity.verticales v
 WHERE v.activo = true
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'contratos', '{}'::jsonb, true
  FROM identity.packs p
 WHERE p.activo = true
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO NOTHING;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, 'contratos', '{}'::jsonb, true
  FROM identity.empresas e
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

CREATE TABLE IF NOT EXISTS orders.contratos (
  id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id       uuid NOT NULL,
  order_id         uuid REFERENCES orders.orden_vista(id) ON DELETE SET NULL,
  parte_id         uuid,
  item_id          uuid,
  tipo             text NOT NULL DEFAULT 'otro'
                   CHECK (tipo IN ('alquiler', 'servicio', 'compra', 'otro')),
  titulo           text NOT NULL CHECK (char_length(trim(titulo)) BETWEEN 2 AND 200),
  descripcion      text NOT NULL DEFAULT '',
  estado           text NOT NULL DEFAULT 'borrador'
                   CHECK (estado IN ('borrador', 'activo', 'finalizado', 'cancelado')),
  inicia_en        date,
  termina_en       date,
  deposito_cents   integer NOT NULL DEFAULT 0 CHECK (deposito_cents >= 0),
  garantia_cents   integer NOT NULL DEFAULT 0 CHECK (garantia_cents >= 0),
  retenido_cents   integer NOT NULL DEFAULT 0 CHECK (retenido_cents >= 0),
  moneda           text NOT NULL DEFAULT 'NIO' CHECK (moneda ~ '^[A-Z]{3}$'),
  metadata         jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  creado_por       uuid NOT NULL,
  creado_en        timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en   timestamptz NOT NULL DEFAULT NOW(),
  finalizado_en    timestamptz,
  CONSTRAINT contratos_retenido_check CHECK (retenido_cents <= deposito_cents),
  CONSTRAINT contratos_fechas_check CHECK (termina_en IS NULL OR inicia_en IS NULL OR termina_en >= inicia_en)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_contratos_order
  ON orders.contratos (order_id)
  WHERE order_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_contratos_empresa_estado
  ON orders.contratos (empresa_id, estado, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_contratos_parte
  ON orders.contratos (empresa_id, parte_id)
  WHERE parte_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS orders.contrato_eventos (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  contrato_id    uuid NOT NULL REFERENCES orders.contratos(id) ON DELETE CASCADE,
  tipo           text NOT NULL
                 CHECK (tipo IN ('deposito_recibido', 'deposito_devuelto', 'retencion', 'ajuste', 'cancelacion')),
  monto_cents    integer NOT NULL DEFAULT 0 CHECK (monto_cents >= 0),
  motivo         text NOT NULL DEFAULT '',
  creado_por     uuid NOT NULL,
  creado_en      timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_contrato_eventos_contrato
  ON orders.contrato_eventos (contrato_id, creado_en);

COMMENT ON TABLE orders.contratos IS
  'Contrato con deposito/garantia y retencion; puede vincularse a una orden.';
COMMENT ON TABLE orders.contrato_eventos IS
  'Historial de deposito, devolucion, retencion y cancelacion del contrato.';

GRANT SELECT, INSERT, UPDATE, DELETE ON orders.contratos TO core_engine_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON orders.contrato_eventos TO core_engine_app;

ALTER TABLE orders.contratos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS contratos_admin ON orders.contratos;
CREATE POLICY contratos_admin ON orders.contratos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS contratos_empresa ON orders.contratos;
CREATE POLICY contratos_empresa ON orders.contratos
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS contratos_lectura ON orders.contratos;
CREATE POLICY contratos_lectura ON orders.contratos
  FOR SELECT TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE orders.contrato_eventos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS contrato_eventos_admin ON orders.contrato_eventos;
CREATE POLICY contrato_eventos_admin ON orders.contrato_eventos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS contrato_eventos_empresa ON orders.contrato_eventos;
CREATE POLICY contrato_eventos_empresa ON orders.contrato_eventos
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS contrato_eventos_lectura ON orders.contrato_eventos;
CREATE POLICY contrato_eventos_lectura ON orders.contrato_eventos
  FOR SELECT TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

COMMIT;
