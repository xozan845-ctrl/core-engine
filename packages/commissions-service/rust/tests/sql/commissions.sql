-- Esquema mínimo de `commissions` para la prueba de integración Rust.
-- Reproduce las columnas finales de 01_esquemas.sql + 01e_adr25_empresas.sql.
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE SCHEMA IF NOT EXISTS commissions;

CREATE TABLE IF NOT EXISTS commissions.comisiones (
  id                       uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id                 uuid NOT NULL,
  linea_clave              text,
  empresa_id               uuid,
  tienda_id                uuid,
  vendedor_id              uuid NOT NULL,
  venta_cents              integer NOT NULL,
  comision_cents           integer NOT NULL,
  monto_vendedor_cents     integer NOT NULL,
  monto_empresario_cents   integer,
  participacion_porcentaje integer CHECK (participacion_porcentaje BETWEEN 0 AND 100),
  estado                   text NOT NULL DEFAULT 'acreditada'
                             CHECK (estado IN ('acreditada', 'liquidada', 'compensada')),
  creado_en                timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_comisiones_orden_linea
  ON commissions.comisiones (order_id, linea_clave);

CREATE TABLE IF NOT EXISTS commissions.pagos (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id    uuid NOT NULL UNIQUE,
  monto_cents integer NOT NULL,
  estado      text NOT NULL DEFAULT 'procesado',
  metodo      text NOT NULL DEFAULT 'simulado',
  creado_en   timestamptz NOT NULL DEFAULT NOW()
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
  periodo_inicio date NOT NULL,
  periodo_fin    date NOT NULL,
  monto_cents    integer NOT NULL,
  estado         text NOT NULL DEFAULT 'aprobada'
                   CHECK (estado IN ('aprobada', 'pagada')),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, periodo_inicio)
);

CREATE TABLE IF NOT EXISTS commissions.compensaciones_devoluciones (
  id                     uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  comision_id            uuid REFERENCES commissions.comisiones(id),
  order_id               uuid NOT NULL,
  vendedor_id            uuid NOT NULL,
  empresa_id             uuid,
  monto_cents            integer NOT NULL,
  monto_empresario_cents integer,
  motivo                 text NOT NULL,
  creado_en              timestamptz NOT NULL DEFAULT NOW()
);

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
