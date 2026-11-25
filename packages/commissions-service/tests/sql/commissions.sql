-- Esquema mínimo de `commissions` para la prueba de integración Rust.
-- Reproduce las columnas finales de 01_esquemas.sql + 01e_adr25_empresas.sql.
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE SCHEMA IF NOT EXISTS commissions;

CREATE TABLE IF NOT EXISTS commissions.comisiones (
  id                       uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id                 uuid NOT NULL,
  linea_clave              text,
  empresa_id               uuid,
  negocio_id               uuid,
  tienda_id                uuid,
  vendedor_id              uuid NOT NULL,
  venta_cents              integer NOT NULL,
  comision_cents           integer NOT NULL,
  monto_vendedor_cents     integer NOT NULL,
  monto_empresario_cents   integer,
  participacion_porcentaje integer CHECK (participacion_porcentaje BETWEEN 0 AND 100),
  cantidad                 integer NOT NULL DEFAULT 1,
  estado                   text NOT NULL DEFAULT 'acreditada'
                             CHECK (estado IN ('acreditada', 'liquidada', 'compensada')),
  creado_en                timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_comisiones_orden_linea
  ON commissions.comisiones (order_id, linea_clave);
ALTER TABLE commissions.comisiones ADD COLUMN IF NOT EXISTS negocio_id uuid;

CREATE TABLE IF NOT EXISTS commissions.pagos (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id    uuid NOT NULL UNIQUE,
  monto_cents integer NOT NULL,
  estado      text NOT NULL DEFAULT 'procesado',
  metodo      text NOT NULL DEFAULT 'simulado',
  creado_en   timestamptz NOT NULL DEFAULT NOW()
);
ALTER TABLE commissions.pagos ADD COLUMN IF NOT EXISTS proveedor text NOT NULL DEFAULT '';
ALTER TABLE commissions.pagos ADD COLUMN IF NOT EXISTS moneda text NOT NULL DEFAULT '';
ALTER TABLE commissions.pagos ADD COLUMN IF NOT EXISTS comision_pasarela_cents integer NOT NULL DEFAULT 0;
ALTER TABLE commissions.pagos ADD COLUMN IF NOT EXISTS impuesto_pasarela_cents integer NOT NULL DEFAULT 0;
ALTER TABLE commissions.pagos ADD COLUMN IF NOT EXISTS retencion_cents integer NOT NULL DEFAULT 0;
ALTER TABLE commissions.pagos ADD COLUMN IF NOT EXISTS monto_liquidado_cents integer NOT NULL DEFAULT 0;
ALTER TABLE commissions.pagos ADD COLUMN IF NOT EXISTS moneda_liquidacion text NOT NULL DEFAULT '';

CREATE TABLE IF NOT EXISTS commissions.configuracion_dispersion (
  id               smallint PRIMARY KEY DEFAULT 1,
  proveedor_codigo text NOT NULL DEFAULT 'tilopay',
  activo           boolean NOT NULL DEFAULT false,
  hora_local       text NOT NULL DEFAULT '00:10',
  minimo_cents     integer NOT NULL DEFAULT 0,
  moneda           text NOT NULL DEFAULT 'NIO',
  retencion_bps    integer NOT NULL DEFAULT 0,
  actualizado_en   timestamptz NOT NULL DEFAULT NOW()
);
INSERT INTO commissions.configuracion_dispersion (id) VALUES (1)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS commissions.cuentas_pago (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  beneficiario_tipo   text NOT NULL,
  beneficiario_id     uuid NOT NULL,
  proveedor           text NOT NULL,
  referencia_externa  text NOT NULL DEFAULT '',
  moneda              text NOT NULL DEFAULT 'NIO',
  detalle             jsonb NOT NULL DEFAULT '{}'::jsonb,
  activo              boolean NOT NULL DEFAULT true,
  creado_en           timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (beneficiario_tipo, beneficiario_id, proveedor)
);

CREATE TABLE IF NOT EXISTS commissions.payouts (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  fecha               date NOT NULL,
  beneficiario_tipo   text NOT NULL,
  beneficiario_id     uuid NOT NULL,
  empresa_id          uuid,
  negocio_id          uuid,
  monto_cents         integer NOT NULL,
  retencion_cents     integer NOT NULL DEFAULT 0,
  monto_neto_cents    integer NOT NULL,
  moneda              text NOT NULL DEFAULT 'NIO',
  proveedor           text NOT NULL,
  estado              text NOT NULL DEFAULT 'pendiente',
  referencia_externa  text NOT NULL DEFAULT '',
  error               text NOT NULL DEFAULT '',
  creado_en           timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (beneficiario_tipo, beneficiario_id, fecha)
);

CREATE TABLE IF NOT EXISTS commissions.payout_items (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  payout_id         uuid NOT NULL REFERENCES commissions.payouts(id) ON DELETE CASCADE,
  liquidacion_tipo  text NOT NULL,
  liquidacion_id    uuid NOT NULL,
  monto_cents       integer NOT NULL,
  UNIQUE (payout_id, liquidacion_tipo, liquidacion_id)
);

CREATE TABLE IF NOT EXISTS commissions.liquidaciones (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  vendedor_id    uuid NOT NULL,
  periodo_inicio date NOT NULL,
  periodo_fin    date NOT NULL,
  monto_cents    integer NOT NULL,
  estado         text NOT NULL DEFAULT 'aprobada'
                   CHECK (estado IN ('aprobada', 'pagada')),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (vendedor_id, periodo_inicio)
);

CREATE TABLE IF NOT EXISTS commissions.liquidaciones_empresas (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  negocio_id     uuid,
  periodo_inicio date NOT NULL,
  periodo_fin    date NOT NULL,
  monto_cents    integer NOT NULL,
  estado         text NOT NULL DEFAULT 'aprobada'
                   CHECK (estado IN ('aprobada', 'pagada')),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, periodo_inicio)
);
ALTER TABLE commissions.liquidaciones_empresas ADD COLUMN IF NOT EXISTS negocio_id uuid;
ALTER TABLE commissions.liquidaciones_empresas
  DROP CONSTRAINT IF EXISTS liquidaciones_empresas_empresa_id_periodo_inicio_key;
CREATE UNIQUE INDEX IF NOT EXISTS idx_liquidaciones_empresa_negocio_periodo
  ON commissions.liquidaciones_empresas (empresa_id,
    COALESCE(negocio_id, '00000000-0000-0000-0000-000000000000'::uuid),
    periodo_inicio);

CREATE TABLE IF NOT EXISTS commissions.compensaciones_devoluciones (
  id                     uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  comision_id            uuid REFERENCES commissions.comisiones(id),
  order_id               uuid NOT NULL,
  vendedor_id            uuid NOT NULL,
  empresa_id             uuid,
  negocio_id             uuid,
  monto_cents            integer NOT NULL,
  monto_empresario_cents integer,
  monto_comision_cents   integer NOT NULL DEFAULT 0,
  cantidad               integer NOT NULL DEFAULT 0,
  motivo                 text NOT NULL,
  creado_en              timestamptz NOT NULL DEFAULT NOW()
);
ALTER TABLE commissions.compensaciones_devoluciones ADD COLUMN IF NOT EXISTS negocio_id uuid;
ALTER TABLE commissions.compensaciones_devoluciones ADD COLUMN IF NOT EXISTS monto_comision_cents integer NOT NULL DEFAULT 0;
ALTER TABLE commissions.compensaciones_devoluciones ADD COLUMN IF NOT EXISTS cantidad integer NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS commissions.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS commissions.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente'
                 CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);

CREATE SCHEMA IF NOT EXISTS identity;
CREATE TABLE IF NOT EXISTS identity.negocios (
  id     uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  nombre text NOT NULL
);

ALTER TABLE commissions.comisiones
  ADD COLUMN IF NOT EXISTS cantidad integer NOT NULL DEFAULT 1;
