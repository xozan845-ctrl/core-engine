-- Core Engine · BodegaHub
-- Esquema field: dominio de logistica de campo (app-test). Multi-tenancy por
-- tenant_id (AD-09): cada organizacion/logistica es un tenant y TODAS las
-- consultas se filtran por tenant_id (defensa en profundidad junto con RLS).
-- El realtime lo provee Supabase al escribir/leer estas tablas.

CREATE SCHEMA IF NOT EXISTS field;

-- ── personal (PersonalLogistica) ──────────────────────────────────────────
CREATE TABLE IF NOT EXISTS field.personal (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  nombre              text NOT NULL,
  apellido            text NOT NULL DEFAULT '',
  cargo               text NOT NULL CHECK (cargo IN ('conductor', 'auxiliar', 'supervisor', 'coordinador')) DEFAULT 'conductor',
  estado              text NOT NULL CHECK (estado IN ('activo', 'en_ruta', 'descansando', 'inactivo')) DEFAULT 'activo',
  telefono            text,
  email               text,
  ruta_asignada_id    uuid,
  vehiculo_asignado_id uuid,
  hora_check_in       timestamptz,
  hora_check_out      timestamptz,
  ubicacion_lat       double precision,
  ubicacion_lng       double precision,
  ubicacion_precision double precision,
  ubicacion_ts        timestamptz,
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_personal_tenant ON field.personal (tenant_id);

-- ── clientes (Cliente) ───────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS field.clientes (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  nombre_completo     text NOT NULL,
  tipo_documento      text,
  numero_documento    text,
  tipo_cliente        text NOT NULL CHECK (tipo_cliente IN ('particular', 'minorista', 'mayorista', 'corporativo')) DEFAULT 'particular',
  email               text,
  telefono            text,
  telefono_alternativo text,
  direccion_principal text,
  direccion_secundaria text,
  referencias_direccion text,
  lat                 double precision,
  lng                 double precision,
  notas_adicionales   text,
  url_google_maps     text,
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_clientes_tenant ON field.clientes (tenant_id);

-- ── vehiculos (Vehiculo) ────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS field.vehiculos (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  placa               text NOT NULL,
  tipo                text NOT NULL CHECK (tipo IN ('camioneta', 'furgon', 'camion', 'moto', 'otro')) DEFAULT 'camioneta',
  marca               text,
  modelo              text,
  anio                integer,
  estado              text NOT NULL CHECK (estado IN ('disponible', 'en_ruta', 'mantenimiento', 'fuera_de_servicio')) DEFAULT 'disponible',
  color               text,
  capacidad_carga_kg  double precision,
  numero_chasis       text,
  numero_motor        text,
  tipo_combustible    text,
  vencimiento_seguro  date,
  vencimiento_circulacion date,
  notas_adicionales   text,
  conductor_id        uuid,
  ruta_activa_id      uuid,
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_vehiculos_tenant ON field.vehiculos (tenant_id);

-- ── rutas (Ruta) ─────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS field.rutas (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  nombre              text NOT NULL,
  descripcion         text,
  estado              text NOT NULL CHECK (estado IN ('pendiente', 'en_curso', 'completada', 'cancelada')) DEFAULT 'pendiente',
  personal_asignado_ids jsonb NOT NULL DEFAULT '[]',
  vehiculo_asignado_id uuid,
  fecha_inicio        timestamptz,
  fecha_fin           timestamptz,
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_rutas_tenant ON field.rutas (tenant_id);

-- ── paradas (Parada de una ruta) ─────────────────────────────────────────
CREATE TABLE IF NOT EXISTS field.paradas (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  ruta_id             uuid NOT NULL REFERENCES field.rutas(id) ON DELETE CASCADE,
  orden               integer NOT NULL DEFAULT 0,
  nombre              text NOT NULL,
  direccion           text,
  lat                 double precision,
  lng                 double precision,
  completada          boolean NOT NULL DEFAULT FALSE,
  pedido_id           uuid,
  tipo                text NOT NULL CHECK (tipo IN ('cliente', 'logistica')) DEFAULT 'cliente',
  cliente_id          uuid,
  telefono            text,
  referencias         text,
  horario_atencion    text,
  notas               text,
  tipo_combustible    text,
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_paradas_ruta ON field.paradas (ruta_id);

-- ── pedidos (Pedido de entrega en campo) ─────────────────────────────────
CREATE TABLE IF NOT EXISTS field.pedidos (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  cliente             text NOT NULL DEFAULT '',
  cliente_id          uuid,
  direccion_entrega   text,
  lat                 double precision,
  lng                 double precision,
  estado              text NOT NULL CHECK (estado IN ('pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado')) DEFAULT 'pendiente',
  ruta_id             uuid,
  notas               text,
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_pedidos_tenant ON field.pedidos (tenant_id);
CREATE INDEX IF NOT EXISTS idx_field_pedidos_ruta ON field.pedidos (ruta_id);

-- ── asistencia (RegistroAsistencia) ──────────────────────────────────────
CREATE TABLE IF NOT EXISTS field.asistencia (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  personal_id         uuid NOT NULL,
  tipo                text NOT NULL CHECK (tipo IN ('entrada', 'salida')),
  registrado_en       timestamptz NOT NULL DEFAULT NOW(),
  lat                 double precision,
  lng                 double precision,
  estado_puntualidad  text NOT NULL CHECK (estado_puntualidad IN ('puntual', 'tardanza', 'salida_anticipada', 'salida_tardia')),
  en_sede             boolean NOT NULL DEFAULT FALSE,
  hora_real_llegada   text,
  minutos_retraso_real integer,
  monitoreo_activo    boolean NOT NULL DEFAULT TRUE,
  notas               text,
  justificacion       text,
  fotos_justificacion jsonb NOT NULL DEFAULT '[]',
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_asistencia_tenant ON field.asistencia (tenant_id);
CREATE INDEX IF NOT EXISTS idx_field_asistencia_personal ON field.asistencia (personal_id, registrado_en);

-- ── incidencias (Incidencia) ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS field.incidencias (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  tipo                text NOT NULL CHECK (tipo IN ('mecanica', 'accidente', 'salud', 'clima', 'otro')),
  estado              text NOT NULL CHECK (estado IN ('abierta', 'en_progreso', 'resuelta')) DEFAULT 'abierta',
  descripcion         text NOT NULL,
  personal_id         uuid,
  vehiculo_id         uuid,
  ruta_id             uuid,
  resolucion          text,
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_incidencias_tenant ON field.incidencias (tenant_id);

-- ── tracking (TrackingRecord, GPS) ───────────────────────────────────────
CREATE TABLE IF NOT EXISTS field.tracking (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  personal_id         uuid NOT NULL,
  latitud             double precision NOT NULL,
  longitud            double precision NOT NULL,
  precision           double precision,
  velocidad           double precision,
  rumbo               double precision,
  registrado_en       timestamptz NOT NULL DEFAULT NOW(),
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_tracking_tenant ON field.tracking (tenant_id);
CREATE INDEX IF NOT EXISTS idx_field_tracking_personal ON field.tracking (personal_id, registrado_en);

-- ── visitas (VisitaCliente / telemetria) ─────────────────────────────────
CREATE TABLE IF NOT EXISTS field.visitas (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  personal_id         uuid NOT NULL,
  cliente_id          uuid,
  ruta_id             uuid,
  tipo_actividad      text NOT NULL CHECK (tipo_actividad IN ('impulsacion', 'venta', 'entrega', 'reparto')),
  resultado           text NOT NULL CHECK (resultado IN ('visitado', 'cerca_no_visitado', 'no_visitado')) DEFAULT 'no_visitado',
  lat                 double precision,
  lng                 double precision,
  distancia_al_cliente double precision,
  hora_llegada        text,
  hora_salida         text,
  duracion_minutos    integer,
  notas               text,
  synced              boolean NOT NULL DEFAULT TRUE,
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_field_visitas_tenant ON field.visitas (tenant_id);

-- ── cumplimiento (CumplimientoRuta, metricas) ────────────────────────────
CREATE TABLE IF NOT EXISTS field.cumplimiento (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid NOT NULL,
  ruta_id             uuid NOT NULL,
  fecha               date NOT NULL,
  metricas            jsonb NOT NULL DEFAULT '{}',
  created_at          timestamptz NOT NULL DEFAULT NOW(),
  updated_at          timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (tenant_id, ruta_id, fecha)
);
CREATE INDEX IF NOT EXISTS idx_field_cumplimiento_tenant ON field.cumplimiento (tenant_id);

-- ── soporte de eventos (mismo patron que los demas servicios) ─────────────
CREATE TABLE IF NOT EXISTS field.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS field.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);
CREATE INDEX IF NOT EXISTS idx_field_outbox_pendientes ON field.outbox (estado, creado_en)
  WHERE estado = 'pendiente';
