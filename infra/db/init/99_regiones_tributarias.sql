-- Regiones tributarias por tenant (ADR-0049).
--
-- La región tributaria es una DIMENSIÓN DE CONFIGURACIÓN, no una capacidad
-- booleana: el pack declara qué regiones soporta, el dueño del negocio elige
-- una al crear la empresa y finance la usa como jurisdicción efectiva.
--
-- Nicaragua es el piloto: queda como región activa y predeterminada de los
-- packs existentes. Añadir otro país es insertar una fila en
-- `finance.jurisdicciones`, declararla en `identity.pack_regiones` y, si hace
-- falta, registrar su adaptador electrónico. No se toca código de dominio.
BEGIN;

-- ── 1. Descriptor completo de región sobre el catálogo fiscal ──────────────
-- `finance.jurisdicciones` sigue siendo la fuente de verdad de las reglas
-- fiscales. Se enriquece con los datos que el onboarding y el manifiesto
-- necesitan para presentar la región sin hardcodear un país.
ALTER TABLE finance.jurisdicciones
  ADD COLUMN IF NOT EXISTS pais_iso text,
  ADD COLUMN IF NOT EXISTS subdivision text,
  ADD COLUMN IF NOT EXISTS moneda_iso text,
  ADD COLUMN IF NOT EXISTS locale text NOT NULL DEFAULT 'es',
  ADD COLUMN IF NOT EXISTS zona_horaria text NOT NULL DEFAULT 'UTC',
  ADD COLUMN IF NOT EXISTS documento_fiscal jsonb NOT NULL DEFAULT '{}'::jsonb,
  ADD COLUMN IF NOT EXISTS perfil jsonb NOT NULL DEFAULT '{}'::jsonb,
  ADD COLUMN IF NOT EXISTS vigente_desde date;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'jurisdicciones_documento_fiscal_check'
  ) THEN
    ALTER TABLE finance.jurisdicciones
      ADD CONSTRAINT jurisdicciones_documento_fiscal_check
      CHECK (jsonb_typeof(documento_fiscal) = 'object');
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'jurisdicciones_perfil_check'
  ) THEN
    ALTER TABLE finance.jurisdicciones
      ADD CONSTRAINT jurisdicciones_perfil_check
      CHECK (jsonb_typeof(perfil) = 'object');
  END IF;
END $$;

-- Nicaragua (piloto). `moneda` conserva el símbolo histórico que usan los
-- comprobantes; `moneda_iso` es el código ISO que usa identity/orders.
UPDATE finance.jurisdicciones
   SET pais_iso = COALESCE(NULLIF(pais_iso, ''), 'NI'),
       moneda_iso = COALESCE(NULLIF(moneda_iso, ''), 'NIO'),
       locale = CASE WHEN locale = 'es' THEN 'es-NI' ELSE locale END,
       zona_horaria = CASE
         WHEN zona_horaria = 'UTC' THEN 'America/Managua'
         ELSE zona_horaria
       END,
       documento_fiscal = CASE
         WHEN documento_fiscal = '{}'::jsonb THEN jsonb_build_object(
           'tipo', 'RUC',
           'nombre', 'Registro Unico de Contribuyente',
           'emisor', 'DGI',
           'etiqueta', 'RUC',
           'descripcion', 'Identificacion fiscal del contribuyente'
         )
         ELSE documento_fiscal
       END,
       perfil = CASE
         WHEN perfil = '{}'::jsonb THEN jsonb_build_object(
           'moneda_iso', 'NIO',
           'locale', 'es-NI',
           'zona_horaria', 'America/Managua',
           'features', jsonb_build_array(
             'fiscal.iva',
             'fiscal.ir',
             'fiscal.declaraciones',
             'fiscal.comprobantes',
             'fiscal.facturacion_electronica',
             'fiscal.notas_credito',
             'fiscal.retenciones'
           )
         )
         ELSE perfil
       END,
       vigente_desde = COALESCE(vigente_desde, DATE '2026-01-01')
 WHERE codigo_pais = 'NI';

CREATE INDEX IF NOT EXISTS idx_jurisdicciones_activas
  ON finance.jurisdicciones (activa, codigo_pais);

COMMENT ON COLUMN finance.jurisdicciones.codigo_pais IS
  'Código de la región tributaria. ISO 3166-1 alpha-2 (NI) o código compuesto para subdivisiones futuras (p. ej. US-CA).';
COMMENT ON COLUMN finance.jurisdicciones.pais_iso IS
  'País ISO 3166-1 alpha-2 al que pertenece la región.';
COMMENT ON COLUMN finance.jurisdicciones.subdivision IS
  'Subdivisión administrativa opcional (estado/provincia) para regiones subnacionales.';
COMMENT ON COLUMN finance.jurisdicciones.moneda_iso IS
  'Código ISO 4217 de la moneda de operación (NIO).';
COMMENT ON COLUMN finance.jurisdicciones.perfil IS
  'Descriptor extensible de la región: features fiscales, presentación y metadatos.';

-- ── 2. Regiones soportadas por pack ────────────────────────────────────────
CREATE TABLE IF NOT EXISTS identity.pack_regiones (
  pack_codigo     text NOT NULL,
  pack_version    integer NOT NULL,
  region_codigo   text NOT NULL REFERENCES finance.jurisdicciones(codigo_pais),
  predeterminada  boolean NOT NULL DEFAULT false,
  activo          boolean NOT NULL DEFAULT true,
  config          jsonb NOT NULL DEFAULT '{}'::jsonb
    CHECK (jsonb_typeof(config) = 'object'),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (pack_codigo, pack_version, region_codigo),
  FOREIGN KEY (pack_codigo, pack_version)
    REFERENCES identity.packs(codigo, version) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_pack_regiones_predeterminada
  ON identity.pack_regiones (pack_codigo, pack_version)
  WHERE predeterminada AND activo;
CREATE INDEX IF NOT EXISTS idx_pack_regiones_region
  ON identity.pack_regiones (region_codigo, activo);

-- Los packs existentes heredan la región piloto (Nicaragua). Si en el futuro
-- hay más regiones activas, un pack nuevo debe declararlas explícitamente.
INSERT INTO identity.pack_regiones
  (pack_codigo, pack_version, region_codigo, predeterminada, activo)
SELECT p.codigo, p.version, j.codigo_pais, true, true
  FROM identity.packs p
 CROSS JOIN finance.jurisdicciones j
 WHERE p.activo = true
   AND j.activa = true
   AND j.codigo_pais = 'NI'
ON CONFLICT (pack_codigo, pack_version, region_codigo) DO NOTHING;

-- ── 3. Selección de la empresa ────────────────────────────────────────────
ALTER TABLE identity.empresas
  ADD COLUMN IF NOT EXISTS region_tributaria text;
ALTER TABLE identity.empresas
  ADD COLUMN IF NOT EXISTS region_tributaria_desde date;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'empresas_region_tributaria_fk'
  ) THEN
    ALTER TABLE identity.empresas
      ADD CONSTRAINT empresas_region_tributaria_fk
      FOREIGN KEY (region_tributaria)
      REFERENCES finance.jurisdicciones(codigo_pais);
  END IF;
END $$;

-- Backfill conservador: las empresas existentes quedan en la región piloto.
UPDATE identity.empresas
   SET region_tributaria = 'NI',
       region_tributaria_desde = COALESCE(region_tributaria_desde, CURRENT_DATE)
 WHERE region_tributaria IS NULL
   AND EXISTS (
     SELECT 1 FROM finance.jurisdicciones
      WHERE codigo_pais = 'NI' AND activa
   );

CREATE INDEX IF NOT EXISTS idx_empresas_region
  ON identity.empresas (region_tributaria);

-- ── 4. Historial de cambios de región (auditoría fiscal) ──────────────────
CREATE TABLE IF NOT EXISTS identity.empresa_region_historial (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  region_anterior text REFERENCES finance.jurisdicciones(codigo_pais),
  region_nueva    text NOT NULL REFERENCES finance.jurisdicciones(codigo_pais),
  motivo          text NOT NULL DEFAULT '' CHECK (char_length(motivo) <= 300),
  actor_id        uuid REFERENCES identity.usuarios(id),
  creado_en       timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_empresa_region_historial
  ON identity.empresa_region_historial (empresa_id, creado_en DESC);

-- ── 5. Privilegios y RLS ──────────────────────────────────────────────────
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA identity, finance TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.pack_regiones TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.empresa_region_historial TO core_engine_app;
  END IF;
END $$;

-- Catálogo de plataforma: lectura para cualquier autenticado, escritura admin.
ALTER TABLE identity.pack_regiones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS pack_regiones_lectura ON identity.pack_regiones;
DROP POLICY IF EXISTS pack_regiones_admin ON identity.pack_regiones;
CREATE POLICY pack_regiones_lectura ON identity.pack_regiones
  FOR SELECT TO anon, authenticated
  USING (activo AND EXISTS (
    SELECT 1 FROM identity.packs p
     WHERE p.codigo = pack_codigo
       AND p.version = pack_version
       AND p.activo
  ));
CREATE POLICY pack_regiones_admin ON identity.pack_regiones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

-- Historial por empresa. `99z_rls_tenant.sql` también crea la política
-- genérica `tenant_app`; se deja aquí una explícita para que la migración sea
-- autosuficiente si se aplica sola.
ALTER TABLE identity.empresa_region_historial ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS empresa_region_historial_admin ON identity.empresa_region_historial;
DROP POLICY IF EXISTS empresa_region_historial_tenant ON identity.empresa_region_historial;
CREATE POLICY empresa_region_historial_admin ON identity.empresa_region_historial
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY empresa_region_historial_tenant ON identity.empresa_region_historial
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY empresa_region_historial_tenant_insert ON identity.empresa_region_historial
  FOR INSERT TO authenticated
  WITH CHECK (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- Las series fiscales dejan de asumir Nicaragua por defecto. Cada serie debe
-- declarar su región al crearse; las series ya existentes conservan la suya.
ALTER TABLE finance.series_comprobantes ALTER COLUMN jurisdiccion DROP DEFAULT;

COMMIT;
