-- Fase P2.1 · entrega digital de bienes/servicios.
-- El fulfillment `digital` crea un acceso por linea de orden. El acceso tiene
-- token, URL configurada por el vendedor y limites de descarga/expiracion.
BEGIN;

INSERT INTO identity.capacidades (codigo, nombre, modulo) VALUES
  ('fulfillment_digital', 'Entrega digital', 'orders')
ON CONFLICT (codigo) DO NOTHING;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'fulfillment_digital', '{}'::jsonb, true
  FROM identity.verticales v
 WHERE v.activo = true
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'fulfillment_digital', '{}'::jsonb, true
  FROM identity.packs p
 WHERE p.activo = true
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO NOTHING;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, 'fulfillment_digital', '{}'::jsonb, true
  FROM identity.empresas e
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

CREATE TABLE IF NOT EXISTS orders.entregas_digitales (
  id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id           uuid NOT NULL,
  order_id             uuid NOT NULL,
  item_id              uuid NOT NULL,
  sku                  text NOT NULL,
  comprador_id         uuid NOT NULL,
  cliente_parte_id     uuid,
  token                uuid NOT NULL DEFAULT gen_random_uuid(),
  recurso_url          text NOT NULL CHECK (char_length(recurso_url) BETWEEN 1 AND 2048),
  estado               text NOT NULL DEFAULT 'activo'
                       CHECK (estado IN ('activo', 'revocado')),
  descargas            integer NOT NULL DEFAULT 0 CHECK (descargas >= 0),
  max_descargas        integer CHECK (max_descargas IS NULL OR max_descargas >= 1),
  primera_descarga_en  timestamptz,
  ultima_descarga_en   timestamptz,
  expira_en            timestamptz,
  creado_en            timestamptz NOT NULL DEFAULT NOW(),
  CONSTRAINT entregas_digitales_linea_unique UNIQUE (order_id, item_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_entregas_digitales_token
  ON orders.entregas_digitales (token);
CREATE INDEX IF NOT EXISTS idx_entregas_digitales_order
  ON orders.entregas_digitales (order_id);
CREATE INDEX IF NOT EXISTS idx_entregas_digitales_comprador
  ON orders.entregas_digitales (comprador_id, creado_en DESC);

COMMENT ON TABLE orders.entregas_digitales IS
  'Acceso digital por linea de orden; el token es una capacidad de descarga.';

GRANT SELECT, INSERT, UPDATE, DELETE ON orders.entregas_digitales TO core_engine_app;

ALTER TABLE orders.entregas_digitales ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS entregas_digitales_admin ON orders.entregas_digitales;
CREATE POLICY entregas_digitales_admin ON orders.entregas_digitales
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS entregas_digitales_comprador ON orders.entregas_digitales;
CREATE POLICY entregas_digitales_comprador ON orders.entregas_digitales
  FOR SELECT TO authenticated
  USING (
    auth.rol() = 'comprador'
    AND comprador_id = auth.uid()
  );
DROP POLICY IF EXISTS entregas_digitales_app ON orders.entregas_digitales;
CREATE POLICY entregas_digitales_app ON orders.entregas_digitales
  FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

COMMIT;
