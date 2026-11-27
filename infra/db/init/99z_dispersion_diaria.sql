-- Dispersión diaria y cuentas de pago (ADR-0053).
--
-- La plataforma cobra en su cuenta, retiene su comisión y paga diariamente al
-- negocio y al vendedor. Las liquidaciones diarias nacen del ledger existente
-- (`commissions.comisiones` + `commissions.liquidaciones*`) y cada payout se
-- envía al proveedor de dispersión configurado (Tilopay o Pagadito).
BEGIN;

-- Desglose que la pasarela reportó para el pago, útil para conciliación.
ALTER TABLE commissions.pagos
  ADD COLUMN IF NOT EXISTS proveedor text NOT NULL DEFAULT '';
ALTER TABLE commissions.pagos
  ADD COLUMN IF NOT EXISTS moneda text NOT NULL DEFAULT '';
ALTER TABLE commissions.pagos
  ADD COLUMN IF NOT EXISTS comision_pasarela_cents integer NOT NULL DEFAULT 0;
ALTER TABLE commissions.pagos
  ADD COLUMN IF NOT EXISTS impuesto_pasarela_cents integer NOT NULL DEFAULT 0;
ALTER TABLE commissions.pagos
  ADD COLUMN IF NOT EXISTS retencion_cents integer NOT NULL DEFAULT 0;
ALTER TABLE commissions.pagos
  ADD COLUMN IF NOT EXISTS monto_liquidado_cents integer NOT NULL DEFAULT 0;
ALTER TABLE commissions.pagos
  ADD COLUMN IF NOT EXISTS moneda_liquidacion text NOT NULL DEFAULT '';

-- Configuración única de dispersión. La plataforma elige el proveedor; las
-- credenciales viven en variables de entorno, nunca en la tabla.
CREATE TABLE IF NOT EXISTS commissions.configuracion_dispersion (
  id              smallint PRIMARY KEY DEFAULT 1 CHECK (id = 1),
  proveedor_codigo text NOT NULL DEFAULT 'tilopay'
                   CHECK (proveedor_codigo IN ('tilopay', 'pagadito')),
  activo          boolean NOT NULL DEFAULT false,
  hora_local      text NOT NULL DEFAULT '00:10'
                   CHECK (hora_local ~ '^([01][0-9]|2[0-3]):[0-5][0-9]$'),
  minimo_cents    integer NOT NULL DEFAULT 0 CHECK (minimo_cents >= 0),
  moneda          text NOT NULL DEFAULT 'NIO' CHECK (moneda ~ '^[A-Z]{3}$'),
  retencion_bps   integer NOT NULL DEFAULT 0
                   CHECK (retencion_bps BETWEEN 0 AND 10000),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);

INSERT INTO commissions.configuracion_dispersion (id)
VALUES (1)
ON CONFLICT (id) DO NOTHING;

-- Cuenta de destino por beneficiario. `referencia_externa` es el token o id
-- que entiende el proveedor; los datos bancarios completos, si hicieran falta,
-- van en `detalle` y no se exponen en la API.
CREATE TABLE IF NOT EXISTS commissions.cuentas_pago (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  beneficiario_tipo   text NOT NULL
                       CHECK (beneficiario_tipo IN ('empresa', 'negocio', 'vendedor')),
  beneficiario_id     uuid NOT NULL,
  proveedor           text NOT NULL
                       CHECK (proveedor IN ('tilopay', 'pagadito')),
  referencia_externa  text NOT NULL DEFAULT '',
  moneda              text NOT NULL DEFAULT 'NIO' CHECK (moneda ~ '^[A-Z]{3}$'),
  detalle             jsonb NOT NULL DEFAULT '{}'::jsonb,
  activo              boolean NOT NULL DEFAULT true,
  creado_en           timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (beneficiario_tipo, beneficiario_id, proveedor)
);

CREATE TABLE IF NOT EXISTS commissions.payouts (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  fecha               date NOT NULL,
  beneficiario_tipo   text NOT NULL
                       CHECK (beneficiario_tipo IN ('empresa', 'negocio', 'vendedor')),
  beneficiario_id     uuid NOT NULL,
  empresa_id          uuid,
  negocio_id          uuid,
  monto_cents         integer NOT NULL CHECK (monto_cents > 0),
  retencion_cents     integer NOT NULL DEFAULT 0 CHECK (retencion_cents >= 0),
  monto_neto_cents    integer NOT NULL CHECK (monto_neto_cents >= 0),
  moneda              text NOT NULL DEFAULT 'NIO' CHECK (moneda ~ '^[A-Z]{3}$'),
  proveedor           text NOT NULL,
  estado              text NOT NULL DEFAULT 'pendiente'
                       CHECK (estado IN ('pendiente', 'enviado', 'confirmado', 'fallido', 'cancelado')),
  referencia_externa  text NOT NULL DEFAULT '',
  error               text NOT NULL DEFAULT '',
  creado_en           timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (beneficiario_tipo, beneficiario_id, fecha)
);

CREATE TABLE IF NOT EXISTS commissions.payout_items (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  payout_id         uuid NOT NULL REFERENCES commissions.payouts(id) ON DELETE CASCADE,
  liquidacion_tipo  text NOT NULL CHECK (liquidacion_tipo IN ('empresa', 'vendedor')),
  liquidacion_id    uuid NOT NULL,
  monto_cents       integer NOT NULL CHECK (monto_cents > 0),
  UNIQUE (payout_id, liquidacion_tipo, liquidacion_id)
);

CREATE INDEX IF NOT EXISTS idx_payouts_fecha_estado
  ON commissions.payouts (fecha DESC, estado);
CREATE INDEX IF NOT EXISTS idx_payouts_beneficiario
  ON commissions.payouts (beneficiario_tipo, beneficiario_id, fecha DESC);
CREATE INDEX IF NOT EXISTS idx_payout_items_liquidacion
  ON commissions.payout_items (liquidacion_tipo, liquidacion_id);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE
      ON commissions.configuracion_dispersion,
         commissions.cuentas_pago,
         commissions.payouts,
         commissions.payout_items
      TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE
      ON commissions.pagos TO core_engine_app;
  END IF;
END $$;

COMMIT;
