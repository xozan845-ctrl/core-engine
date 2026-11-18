-- Cupones de descuento por empresa (fase retail).
--
-- El cupón es una regla de pricing: código, vigencia, mínimo de compra, tope de
-- descuento y usos. `pricing.cupon_usos` registra el canje por orden para que
-- sea idempotente y auditable.
BEGIN;

CREATE TABLE IF NOT EXISTS pricing.cupones (
  id                       uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id               uuid NOT NULL,
  codigo                   text NOT NULL CHECK (codigo ~ '^[A-Z0-9][A-Z0-9_-]{2,31}$'),
  nombre                   text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  tipo                     text NOT NULL CHECK (tipo IN ('porcentaje', 'monto_fijo')),
  valor                    integer NOT NULL CHECK (valor > 0),
  minimo_compra_cents      integer NOT NULL DEFAULT 0 CHECK (minimo_compra_cents >= 0),
  maximo_descuento_cents   integer CHECK (maximo_descuento_cents IS NULL OR maximo_descuento_cents > 0),
  usos_maximos             integer CHECK (usos_maximos IS NULL OR usos_maximos > 0),
  usos_actuales            integer NOT NULL DEFAULT 0 CHECK (usos_actuales >= 0),
  fecha_desde              date NOT NULL,
  fecha_hasta              date NOT NULL,
  activo                   boolean NOT NULL DEFAULT true,
  creado_en                timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en           timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en             timestamptz,
  CHECK (fecha_hasta >= fecha_desde),
  CHECK (
    (tipo = 'porcentaje' AND valor BETWEEN 1 AND 100)
    OR (tipo = 'monto_fijo')
  )
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_cupones_empresa_codigo
  ON pricing.cupones (empresa_id, upper(codigo))
  WHERE eliminado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_cupones_empresa_vigencia
  ON pricing.cupones (empresa_id, activo, fecha_desde, fecha_hasta)
  WHERE eliminado_en IS NULL;

CREATE TABLE IF NOT EXISTS pricing.cupon_usos (
  id                     uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  cupon_id               uuid NOT NULL REFERENCES pricing.cupones(id) ON DELETE CASCADE,
  empresa_id             uuid NOT NULL,
  order_id               uuid NOT NULL,
  codigo                 text NOT NULL,
  monto_descuento_cents  integer NOT NULL CHECK (monto_descuento_cents >= 0),
  estado                 text NOT NULL DEFAULT 'usado'
                         CHECK (estado IN ('usado', 'liberado')),
  creado_en              timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_cupon_usos_cupon_orden
  ON pricing.cupon_usos (cupon_id, order_id);
CREATE INDEX IF NOT EXISTS idx_cupon_usos_empresa
  ON pricing.cupon_usos (empresa_id, creado_en DESC);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON pricing.cupones TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON pricing.cupon_usos TO core_engine_app;
  END IF;
END $$;

COMMIT;
