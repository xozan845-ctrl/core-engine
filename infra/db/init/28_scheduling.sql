-- Plataforma multi-vertical · agenda y recursos (fase 4).
-- Bounded context compartido por belleza, dental, flota y futuros servicios.
-- La exclusion GiST es la garantia de no-solapamiento bajo concurrencia.
BEGIN;

CREATE EXTENSION IF NOT EXISTS btree_gist;
CREATE SCHEMA IF NOT EXISTS scheduling;

CREATE TABLE IF NOT EXISTS scheduling.recursos (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  tipo            text NOT NULL CHECK (tipo IN ('profesional', 'sala', 'sillon', 'vehiculo', 'otro')),
  nombre          text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 160),
  estado          text NOT NULL DEFAULT 'activo' CHECK (estado IN ('activo', 'inactivo')),
  atributos       jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(atributos) = 'object'),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_scheduling_recursos_empresa_nombre
  ON scheduling.recursos (empresa_id, lower(nombre));
CREATE INDEX IF NOT EXISTS idx_scheduling_recursos_empresa_estado
  ON scheduling.recursos (empresa_id, estado, creado_en DESC);

CREATE TABLE IF NOT EXISTS scheduling.disponibilidad (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  recurso_id      uuid NOT NULL REFERENCES scheduling.recursos(id) ON DELETE CASCADE,
  dia_semana      smallint NOT NULL CHECK (dia_semana BETWEEN 0 AND 6),
  inicia_hora     time NOT NULL,
  termina_hora    time NOT NULL,
  zona_horaria    text NOT NULL DEFAULT 'America/Managua',
  vigente_desde  date NOT NULL DEFAULT CURRENT_DATE,
  vigente_hasta  date,
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  CHECK (inicia_hora < termina_hora),
  CHECK (vigente_hasta IS NULL OR vigente_hasta >= vigente_desde)
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_scheduling_disponibilidad_intervalo
  ON scheduling.disponibilidad (recurso_id, dia_semana, inicia_hora, termina_hora, vigente_desde);
CREATE INDEX IF NOT EXISTS idx_scheduling_disponibilidad_empresa
  ON scheduling.disponibilidad (empresa_id, recurso_id, dia_semana);

CREATE TABLE IF NOT EXISTS scheduling.bloqueos (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  recurso_id      uuid NOT NULL REFERENCES scheduling.recursos(id) ON DELETE CASCADE,
  periodo         tstzrange NOT NULL,
  motivo          text NOT NULL DEFAULT '',
  creado_por      uuid,
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  CHECK (NOT isempty(periodo)),
  EXCLUDE USING gist (recurso_id WITH =, periodo WITH &&)
);
CREATE INDEX IF NOT EXISTS idx_scheduling_bloqueos_empresa_periodo
  ON scheduling.bloqueos (empresa_id, recurso_id, periodo);

CREATE TABLE IF NOT EXISTS scheduling.citas (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  recurso_id      uuid NOT NULL REFERENCES scheduling.recursos(id),
  cliente_id      uuid,
  order_id        uuid,
  item_id         uuid,
  estado          text NOT NULL DEFAULT 'reservada'
                  CHECK (estado IN ('reservada', 'confirmada', 'en_atencion', 'completada', 'cancelada', 'no_asistio')),
  inicia_en       timestamptz NOT NULL,
  termina_en      timestamptz NOT NULL,
  periodo         tstzrange GENERATED ALWAYS AS (tstzrange(inicia_en, termina_en, '[)')) STORED,
  notas           text NOT NULL DEFAULT '',
  metadata        jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW(),
  CHECK (inicia_en < termina_en),
  EXCLUDE USING gist (recurso_id WITH =, periodo WITH &&)
    WHERE (estado IN ('reservada', 'confirmada', 'en_atencion'))
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_scheduling_citas_order
  ON scheduling.citas (order_id) WHERE order_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_scheduling_citas_empresa_inicio
  ON scheduling.citas (empresa_id, inicia_en DESC);
CREATE INDEX IF NOT EXISTS idx_scheduling_citas_recurso_inicio
  ON scheduling.citas (recurso_id, inicia_en);

CREATE TABLE IF NOT EXISTS scheduling.recordatorios (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  cita_id         uuid NOT NULL REFERENCES scheduling.citas(id) ON DELETE CASCADE,
  tipo            text NOT NULL CHECK (tipo IN ('24h', '2h')),
  programado_en   timestamptz NOT NULL,
  estado          text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'enviado', 'cancelado')),
  enviado_en      timestamptz,
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (cita_id, tipo)
);
CREATE INDEX IF NOT EXISTS idx_scheduling_recordatorios_pendientes
  ON scheduling.recordatorios (estado, programado_en);

CREATE TABLE IF NOT EXISTS scheduling.eventos_procesados (
  event_id        uuid PRIMARY KEY,
  tipo            text NOT NULL,
  procesado_en    timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS scheduling.outbox (
  event_id        uuid PRIMARY KEY,
  tipo            text NOT NULL,
  payload         jsonb NOT NULL,
  estado          text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  publicado_en    timestamptz
);

-- La cuenta de aplicación no es propietaria de los esquemas en staging/local.
-- Conservar el patrón de privilegios de los dominios existentes evita que el
-- bounded context nuevo falle por permisos antes de que RLS evalúe el tenant.
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA scheduling TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA scheduling TO core_engine_app;
  END IF;
END $$;

COMMIT;
