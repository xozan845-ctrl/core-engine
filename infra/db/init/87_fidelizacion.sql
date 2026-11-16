-- Fase 8 · fidelización (capacidad `fidelizacion`): programas, tarjetas,
-- movimientos, recompensas y canjes. Migración aditiva: las empresas sin la
-- capacidad activa no ven ni usan estas tablas.
BEGIN;

CREATE TABLE IF NOT EXISTS orders.fidelizacion_programas (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id          uuid NOT NULL,
  nombre              text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  activo              boolean NOT NULL DEFAULT true,
  puntos_por_unidad   integer NOT NULL DEFAULT 1 CHECK (puntos_por_unidad >= 0),
  expira_meses        integer CHECK (expira_meses IS NULL OR expira_meses > 0),
  config              jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  creado_en           timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id)
);

CREATE TABLE IF NOT EXISTS orders.fidelizacion_tarjetas (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  programa_id     uuid NOT NULL REFERENCES orders.fidelizacion_programas(id) ON DELETE CASCADE,
  parte_id        uuid NOT NULL REFERENCES identity.partes(id) ON DELETE CASCADE,
  codigo          text NOT NULL CHECK (codigo ~ '^[A-Z0-9][A-Z0-9-]{3,31}$'),
  puntos          integer NOT NULL DEFAULT 0 CHECK (puntos >= 0),
  estado          text NOT NULL DEFAULT 'activa'
                  CHECK (estado IN ('activa', 'bloqueada', 'cerrada')),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, parte_id),
  UNIQUE (empresa_id, codigo)
);
CREATE INDEX IF NOT EXISTS idx_fidelizacion_tarjetas_parte
  ON orders.fidelizacion_tarjetas (parte_id);

CREATE TABLE IF NOT EXISTS orders.fidelizacion_movimientos (
  id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id   uuid NOT NULL,
  tarjeta_id   uuid NOT NULL REFERENCES orders.fidelizacion_tarjetas(id) ON DELETE CASCADE,
  tipo         text NOT NULL
               CHECK (tipo IN ('acumulacion', 'canje', 'ajuste', 'expiracion')),
  puntos       integer NOT NULL CHECK (puntos <> 0),
  saldo_tras   integer NOT NULL CHECK (saldo_tras >= 0),
  order_id     uuid,
  event_id     uuid,
  descripcion  text NOT NULL DEFAULT '' CHECK (char_length(descripcion) <= 500),
  creado_por   uuid,
  creado_en    timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_fidelizacion_movimientos_tarjeta
  ON orders.fidelizacion_movimientos (tarjeta_id, creado_en DESC);
-- Idempotencia de la acumulación: una orden suma puntos una sola vez por tarjeta.
CREATE UNIQUE INDEX IF NOT EXISTS ux_fidelizacion_movimientos_acumulacion
  ON orders.fidelizacion_movimientos (tarjeta_id, order_id)
  WHERE tipo = 'acumulacion' AND order_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS orders.fidelizacion_recompensas (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  codigo          text NOT NULL CHECK (codigo ~ '^[a-z0-9][a-z0-9_-]{1,39}$'),
  nombre          text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  descripcion     text NOT NULL DEFAULT '' CHECK (char_length(descripcion) <= 500),
  puntos          integer NOT NULL CHECK (puntos > 0),
  stock           integer CHECK (stock IS NULL OR stock >= 0),
  activo          boolean NOT NULL DEFAULT true,
  config          jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, codigo)
);

CREATE TABLE IF NOT EXISTS orders.fidelizacion_canjes (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  tarjeta_id      uuid NOT NULL REFERENCES orders.fidelizacion_tarjetas(id) ON DELETE CASCADE,
  recompensa_id   uuid NOT NULL REFERENCES orders.fidelizacion_recompensas(id),
  movimiento_id   uuid NOT NULL REFERENCES orders.fidelizacion_movimientos(id),
  order_id        uuid,
  estado          text NOT NULL DEFAULT 'confirmado'
                  CHECK (estado IN ('confirmado', 'anulado')),
  puntos          integer NOT NULL CHECK (puntos > 0),
  creado_por      uuid,
  creado_en       timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_fidelizacion_canjes_tarjeta
  ON orders.fidelizacion_canjes (tarjeta_id, creado_en DESC);

COMMIT;
