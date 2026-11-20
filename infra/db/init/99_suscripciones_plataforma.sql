-- Suscripción de plataforma para expansión de negocios (ADR-0046).
--
-- Regla MVP: el primer negocio/empresa es libre; para operar más de uno se
-- exige una suscripción mensual de USD 35 (plan `empresario_plus`, 3500 cents).
-- El precio y el periodo viven en `identity.planes_plataforma`, no en código.
-- Los empresarios existentes reciben 30 días de gracia (`origen = 'legacy'`).

BEGIN;

CREATE TABLE IF NOT EXISTS identity.planes_plataforma (
  codigo          text PRIMARY KEY,
  nombre          text NOT NULL,
  descripcion     text NOT NULL DEFAULT '',
  precio_cents    integer NOT NULL CHECK (precio_cents >= 0),
  moneda          text NOT NULL DEFAULT 'USD' CHECK (char_length(moneda) = 3),
  periodo_meses   integer NOT NULL DEFAULT 1 CHECK (periodo_meses BETWEEN 1 AND 12),
  limite_negocios integer CHECK (limite_negocios IS NULL OR limite_negocios >= 1),
  activo          boolean NOT NULL DEFAULT true,
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);

INSERT INTO identity.planes_plataforma
  (codigo, nombre, descripcion, precio_cents, moneda, periodo_meses, limite_negocios)
VALUES (
  'empresario_plus',
  'Empresario',
  'Permite operar más de un negocio dentro de la plataforma.',
  3500,
  'USD',
  1,
  NULL
)
ON CONFLICT (codigo) DO NOTHING;

CREATE TABLE IF NOT EXISTS identity.suscripciones_plataforma (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  usuario_id     uuid NOT NULL REFERENCES identity.usuarios (id) ON DELETE CASCADE,
  empresa_id     uuid REFERENCES identity.empresas (id) ON DELETE SET NULL,
  plan_codigo    text NOT NULL REFERENCES identity.planes_plataforma (codigo),
  estado         text NOT NULL DEFAULT 'pendiente'
                   CHECK (estado IN ('pendiente', 'activa', 'vencida', 'cancelada')),
  origen         text NOT NULL DEFAULT 'manual'
                   CHECK (origen IN ('manual', 'pasarela', 'legacy')),
  precio_cents   integer NOT NULL CHECK (precio_cents >= 0),
  moneda         text NOT NULL DEFAULT 'USD' CHECK (char_length(moneda) = 3),
  inicio_en      timestamptz,
  fin_en         timestamptz,
  proximo_cobro  timestamptz,
  proveedor      text NOT NULL DEFAULT 'manual',
  referencia     text NOT NULL DEFAULT '',
  creado_por     uuid REFERENCES identity.usuarios (id) ON DELETE SET NULL,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_suscripciones_plataforma_activa
  ON identity.suscripciones_plataforma (usuario_id)
  WHERE estado = 'activa';

CREATE INDEX IF NOT EXISTS idx_suscripciones_plataforma_usuario
  ON identity.suscripciones_plataforma (usuario_id, creado_en DESC);

CREATE INDEX IF NOT EXISTS idx_suscripciones_plataforma_empresa
  ON identity.suscripciones_plataforma (empresa_id, creado_en DESC);

-- Los empresarios que ya operaban no pierden la capacidad de golpe: se les
-- concede un periodo de gracia de 30 días para regularizar el cobro.
INSERT INTO identity.suscripciones_plataforma
  (usuario_id, empresa_id, plan_codigo, estado, origen, precio_cents, moneda,
   inicio_en, fin_en, proximo_cobro, proveedor, referencia, creado_por)
SELECT
  u.id,
  MIN(e.id::text)::uuid,
  'empresario_plus',
  'activa',
  'legacy',
  3500,
  'USD',
  NOW(),
  NOW() + INTERVAL '30 days',
  NOW() + INTERVAL '30 days',
  'manual',
  'migracion-legacy',
  u.id
FROM identity.usuarios u
JOIN identity.empresas e ON e.creado_por = u.id
WHERE u.rol = 'empresario'
GROUP BY u.id
ON CONFLICT DO NOTHING;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.planes_plataforma TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.suscripciones_plataforma TO core_engine_app;
  END IF;
END $$;

COMMIT;
