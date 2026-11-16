-- ADR-0021 / ADR-0022: analytics-service como único dueño de la analítica.
--
-- `orders-service` publica hechos; `commissions-service` publica comisiones;
-- `analytics-service` mantiene la proyección y expone negocio, empresa y
-- plataforma. Este servicio es la excepción OLAP autorizada.
BEGIN;

CREATE SCHEMA IF NOT EXISTS analytics;

-- ── Hechos de venta (una fila por línea de orden) ─────────────────────────
CREATE TABLE IF NOT EXISTS analytics.hechos_venta (
  id                      uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id                uuid NOT NULL,
  linea_clave             text NOT NULL,
  empresa_id              uuid NOT NULL,
  negocio_id              uuid,
  tienda_id               uuid,
  vendedor_id             uuid,
  comprador_id            uuid,
  cantidad                bigint NOT NULL,
  precio_unitario_cents   bigint NOT NULL,
  venta_cents             bigint NOT NULL,
  orden_creada_en         timestamptz NOT NULL,
  creado_en               timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (order_id, linea_clave)
);

CREATE INDEX IF NOT EXISTS idx_analytics_hechos_empresa
  ON analytics.hechos_venta (empresa_id, negocio_id, orden_creada_en DESC);
CREATE INDEX IF NOT EXISTS idx_analytics_hechos_comprador
  ON analytics.hechos_venta (empresa_id, comprador_id, orden_creada_en);

-- ── Comisiones (una fila por línea de orden) ──────────────────────────────
CREATE TABLE IF NOT EXISTS analytics.comisiones (
  id                        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id                  uuid NOT NULL,
  linea_clave               text NOT NULL,
  empresa_id                uuid NOT NULL,
  negocio_id                uuid,
  tienda_id                 uuid,
  vendedor_id               uuid,
  comision_cents            bigint NOT NULL,
  monto_vendedor_cents      bigint NOT NULL DEFAULT 0,
  monto_empresario_cents    bigint NOT NULL DEFAULT 0,
  participacion_porcentaje  integer,
  creado_en                 timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (order_id, linea_clave)
);

CREATE INDEX IF NOT EXISTS idx_analytics_comisiones_empresa
  ON analytics.comisiones (empresa_id, negocio_id, creado_en DESC);

CREATE TABLE IF NOT EXISTS analytics.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

-- ── Reportes programados ──────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS analytics.reportes_programados (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL,
  negocio_id         uuid,
  nombre             text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  tipo               text NOT NULL DEFAULT 'analitica'
                     CHECK (tipo IN ('analitica', 'cohortes')),
  formato            text NOT NULL DEFAULT 'csv'
                     CHECK (formato IN ('json', 'csv')),
  frecuencia         text NOT NULL
                     CHECK (frecuencia IN ('diaria', 'semanal', 'mensual')),
  destino            text NOT NULL CHECK (char_length(trim(destino)) BETWEEN 3 AND 200),
  activo             boolean NOT NULL DEFAULT true,
  ultima_ejecucion   timestamptz,
  proxima_ejecucion  timestamptz NOT NULL,
  creado_por         uuid NOT NULL,
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_analytics_reportes_pendientes
  ON analytics.reportes_programados (proxima_ejecucion)
  WHERE activo;

CREATE INDEX IF NOT EXISTS idx_analytics_reportes_empresa
  ON analytics.reportes_programados (empresa_id, activo, creado_en DESC);

CREATE TABLE IF NOT EXISTS analytics.reporte_ejecuciones (
  id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  reporte_id    uuid NOT NULL
                REFERENCES analytics.reportes_programados(id) ON DELETE CASCADE,
  empresa_id    uuid NOT NULL,
  negocio_id    uuid,
  tipo          text NOT NULL,
  formato       text NOT NULL,
  estado        text NOT NULL CHECK (estado IN ('ok', 'error')),
  contenido     text NOT NULL DEFAULT '',
  error         text NOT NULL DEFAULT '',
  ejecutada_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_analytics_ejecuciones_reporte
  ON analytics.reporte_ejecuciones (reporte_id, ejecutada_en DESC);

-- ── Backfill de hechos históricos ─────────────────────────────────────────
INSERT INTO analytics.hechos_venta
  (order_id, linea_clave, empresa_id, negocio_id, tienda_id, vendedor_id,
   comprador_id, cantidad, precio_unitario_cents, venta_cents, orden_creada_en)
SELECT c.order_id,
       c.linea_clave,
       c.empresa_id,
       c.negocio_id,
       c.tienda_id,
       c.vendedor_id,
       o.comprador_id,
       COALESCE((item->>'cantidad')::bigint, 0),
       COALESCE((item->>'precio_unitario_cents')::bigint, 0),
       c.venta_cents,
       o.creado_en
  FROM commissions.comisiones c
  JOIN orders.orden_vista o ON o.id = c.order_id
  JOIN LATERAL jsonb_array_elements(o.items_json) item
    ON item->>'oferta_id' = c.linea_clave
 WHERE c.empresa_id IS NOT NULL
   AND c.linea_clave IS NOT NULL
   AND c.estado <> 'compensada'
   AND o.estado NOT IN ('cancelada', 'devuelta')
ON CONFLICT (order_id, linea_clave) DO NOTHING;

INSERT INTO analytics.comisiones
  (order_id, linea_clave, empresa_id, negocio_id, tienda_id, vendedor_id,
   comision_cents, monto_vendedor_cents, monto_empresario_cents,
   participacion_porcentaje, creado_en)
SELECT c.order_id,
       c.linea_clave,
       c.empresa_id,
       c.negocio_id,
       c.tienda_id,
       c.vendedor_id,
       c.comision_cents,
       c.monto_vendedor_cents,
       COALESCE(c.monto_empresario_cents, 0),
       c.participacion_porcentaje,
       c.creado_en
  FROM commissions.comisiones c
 WHERE c.empresa_id IS NOT NULL
   AND c.linea_clave IS NOT NULL
   AND c.estado <> 'compensada'
ON CONFLICT (order_id, linea_clave) DO NOTHING;

-- ── Capacidad de la feature ───────────────────────────────────────────────
INSERT INTO identity.capacidades (codigo, nombre, modulo) VALUES
  ('analitica_negocios', 'Analitica de negocios', 'analytics')
ON CONFLICT (codigo) DO NOTHING;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'analitica_negocios', '{}'::jsonb, true
  FROM identity.verticales v
 WHERE v.activo
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

INSERT INTO identity.pack_capacidades
  (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'analitica_negocios', '{}'::jsonb, true
  FROM identity.packs p
 WHERE p.activo
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO NOTHING;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, 'analitica_negocios', '{}'::jsonb, true
  FROM identity.empresas e
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

GRANT USAGE ON SCHEMA analytics TO core_engine_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA analytics TO core_engine_app;

-- ── RLS ───────────────────────────────────────────────────────────────────
ALTER TABLE analytics.hechos_venta ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS hechos_venta_admin ON analytics.hechos_venta;
CREATE POLICY hechos_venta_admin ON analytics.hechos_venta
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS hechos_venta_empresa ON analytics.hechos_venta;
CREATE POLICY hechos_venta_empresa ON analytics.hechos_venta
  FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario'
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY hechos_venta_app ON analytics.hechos_venta
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE analytics.comisiones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS comisiones_admin ON analytics.comisiones;
CREATE POLICY comisiones_admin ON analytics.comisiones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS comisiones_empresa ON analytics.comisiones;
CREATE POLICY comisiones_empresa ON analytics.comisiones
  FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario'
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY comisiones_app ON analytics.comisiones
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE analytics.reportes_programados ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS reportes_programados_admin ON analytics.reportes_programados;
CREATE POLICY reportes_programados_admin ON analytics.reportes_programados
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS reportes_programados_empresa ON analytics.reportes_programados;
CREATE POLICY reportes_programados_empresa ON analytics.reportes_programados
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario'
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario'
              AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY reportes_programados_app ON analytics.reportes_programados
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE analytics.reporte_ejecuciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS reporte_ejecuciones_admin ON analytics.reporte_ejecuciones;
CREATE POLICY reporte_ejecuciones_admin ON analytics.reporte_ejecuciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS reporte_ejecuciones_empresa ON analytics.reporte_ejecuciones;
CREATE POLICY reporte_ejecuciones_empresa ON analytics.reporte_ejecuciones
  FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario'
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY reporte_ejecuciones_app ON analytics.reporte_ejecuciones
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE analytics.eventos_procesados ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS eventos_procesados_admin ON analytics.eventos_procesados;
CREATE POLICY eventos_procesados_admin ON analytics.eventos_procesados
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY eventos_procesados_app ON analytics.eventos_procesados
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

COMMIT;
