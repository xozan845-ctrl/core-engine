-- Core Engine · Almacenamiento de objetos (storage-service)
-- Esquema del servicio de storage: **un bucket de MinIO por tenant**
-- (multi-tenancy por bucket, ADR-24) y la metadata de cada objeto para cuotas,
-- listados y auditoria. El binario vive en MinIO; aqui vive la verdad de
-- negocio (quien subio que, en que estado y cuanto ocupa).

CREATE SCHEMA IF NOT EXISTS storage;

-- Un bucket por tenant: la fila es la fuente de la cuota y del uso acumulado
-- (usado_bytes lo mantiene el servicio en la misma transaccion del objeto).
CREATE TABLE IF NOT EXISTS storage.buckets (
  tenant_id      uuid PRIMARY KEY,
  bucket         text NOT NULL UNIQUE,
  cuota_bytes    bigint NOT NULL CHECK (cuota_bytes > 0),
  usado_bytes    bigint NOT NULL DEFAULT 0 CHECK (usado_bytes >= 0),
  estado         text NOT NULL DEFAULT 'activo' CHECK (estado IN ('activo', 'suspendido')),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

-- Objetos: 'pendiente' mientras la subida prefirmada no se confirma (no cuenta
-- en la cuota), 'activo' cuando esta en MinIO y cuenta, 'eliminado' en la baja
-- logica (el binario se borra del bucket en el mismo paso).
CREATE TABLE IF NOT EXISTS storage.objetos (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id       uuid NOT NULL,
  bucket          text NOT NULL,
  objeto          text NOT NULL UNIQUE,
  nombre_original text NOT NULL,
  content_type    text NOT NULL,
  carpeta         text NOT NULL DEFAULT 'general',
  descripcion     text NOT NULL DEFAULT '',
  etiquetas       jsonb NOT NULL DEFAULT '{}',
  tamano_bytes    bigint NOT NULL CHECK (tamano_bytes > 0),
  etag            text,
  estado          text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'activo', 'eliminado')),
  subido_por      uuid,
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  confirmado_en   timestamptz,
  eliminado_en    timestamptz
);
CREATE INDEX IF NOT EXISTS idx_storage_objetos_tenant ON storage.objetos (tenant_id, estado, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_storage_objetos_carpeta ON storage.objetos (tenant_id, carpeta);
CREATE INDEX IF NOT EXISTS idx_storage_objetos_pendientes ON storage.objetos (estado, creado_en)
  WHERE estado = 'pendiente';

-- Outbox (R-AR-3): eventos de almacenamiento publicados por el poller central.
CREATE TABLE IF NOT EXISTS storage.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);
CREATE INDEX IF NOT EXISTS idx_storage_outbox_pendientes ON storage.outbox (estado, creado_en)
  WHERE estado = 'pendiente';

-- Idempotencia de eventos consumidos (el servicio hoy es editor; la tabla se
-- crea igual para que el contrato del poller/consumidores sea homogeneo).
CREATE TABLE IF NOT EXISTS storage.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);
