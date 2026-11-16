-- Esquema mínimo de analytics para las pruebas de integración Rust.
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE SCHEMA IF NOT EXISTS analytics;
CREATE SCHEMA IF NOT EXISTS identity;

CREATE TABLE IF NOT EXISTS identity.empresas (
  id     uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  nombre text NOT NULL
);

CREATE TABLE IF NOT EXISTS identity.negocios (
  id     uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  nombre text NOT NULL
);

CREATE TABLE IF NOT EXISTS analytics.hechos_venta (
  id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id              uuid NOT NULL,
  linea_clave           text NOT NULL,
  empresa_id            uuid NOT NULL,
  negocio_id            uuid,
  tienda_id             uuid,
  vendedor_id           uuid,
  comprador_id          uuid,
  cantidad              bigint NOT NULL,
  precio_unitario_cents bigint NOT NULL,
  venta_cents           bigint NOT NULL,
  orden_creada_en       timestamptz NOT NULL,
  creado_en             timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (order_id, linea_clave)
);

CREATE TABLE IF NOT EXISTS analytics.comisiones (
  id                       uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id                 uuid NOT NULL,
  linea_clave              text NOT NULL,
  empresa_id               uuid NOT NULL,
  negocio_id               uuid,
  tienda_id                uuid,
  vendedor_id              uuid,
  comision_cents           bigint NOT NULL,
  monto_vendedor_cents     bigint NOT NULL DEFAULT 0,
  monto_empresario_cents   bigint NOT NULL DEFAULT 0,
  participacion_porcentaje integer,
  creado_en                timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (order_id, linea_clave)
);

CREATE TABLE IF NOT EXISTS analytics.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS analytics.reportes_programados (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id        uuid NOT NULL,
  negocio_id        uuid,
  nombre            text NOT NULL,
  tipo              text NOT NULL,
  formato           text NOT NULL,
  frecuencia        text NOT NULL,
  destino           text NOT NULL,
  activo            boolean NOT NULL DEFAULT true,
  ultima_ejecucion  timestamptz,
  proxima_ejecucion timestamptz NOT NULL,
  creado_por        uuid NOT NULL,
  creado_en         timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en    timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS analytics.reporte_ejecuciones (
  id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  reporte_id   uuid NOT NULL REFERENCES analytics.reportes_programados(id) ON DELETE CASCADE,
  empresa_id   uuid NOT NULL,
  negocio_id   uuid,
  tipo         text NOT NULL,
  formato      text NOT NULL,
  estado       text NOT NULL,
  contenido    text NOT NULL DEFAULT '',
  error        text NOT NULL DEFAULT '',
  ejecutada_en timestamptz NOT NULL DEFAULT NOW()
);
