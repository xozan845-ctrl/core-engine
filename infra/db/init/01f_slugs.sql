-- ADR-26: slugs publicos para empresas y tiendas.
-- Migracion aditiva: el UUID sigue siendo la clave interna y sigue resolviendo
-- las rutas existentes; el slug es una clave publica estable que puede cambiar
-- sin romper enlaces (los slugs anteriores redirigen 301 al actual).
BEGIN;

-- ── normalizacion compartida (identica a packages/shared/src/slug.ts) ──────
CREATE OR REPLACE FUNCTION public.fn_normalizar_slug(valor text)
RETURNS text
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
  SELECT NULLIF(
    rtrim(
      left(
        trim(both '-' FROM regexp_replace(
          regexp_replace(
            translate(lower(valor),
              'áàäâãéèëêíìïîóòöôõúùüûñç',
              'aaaaaeeeeiiiiooooouuuunc'),
            '[^a-z0-9]+', '-', 'g'),
          '-+', '-', 'g')
        ),
        80
      ),
      '-'
    ),
    ''
  );
$$;

-- Misma lista que SLUGS_RESERVADOS en packages/shared/src/slug.ts.
CREATE OR REPLACE FUNCTION public.fn_slug_reservado(valor text)
RETURNS boolean
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
  SELECT valor = ANY (ARRAY[
    'admin', 'administrador', 'api', 'app', 'auth', 'carrito', 'catalog',
    'docs', 'empresa', 'empresas', 'empresario', 'empresarios', 'envios',
    'health', 'internal', 'login', 'logout', 'me', 'metrics', 'mis', 'nuevo',
    'nueva', 'ofertas', 'orders', 'pagos', 'productos', 'public', 'ready',
    'registro', 'reportes', 'shop', 'static', 'tienda', 'tiendas', 'todas',
    'todos', 'vendedor', 'vendedores'
  ]);
$$;

CREATE OR REPLACE FUNCTION public.fn_slug_con_sufijo(base text, numero integer)
RETURNS text
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
  SELECT COALESCE(
    NULLIF(rtrim(left(base, GREATEST(2, 80 - char_length('-' || numero::text))), '-'), ''),
    'entidad'
  ) || '-' || numero::text;
$$;

CREATE OR REPLACE FUNCTION public.fn_slug_desde_nombre(nombre text, fallback text DEFAULT 'entidad')
RETURNS text
LANGUAGE plpgsql
IMMUTABLE
AS $$
DECLARE
  base text;
BEGIN
  base := public.fn_normalizar_slug(COALESCE(nombre, ''));
  IF base IS NULL THEN
    base := public.fn_normalizar_slug(COALESCE(fallback, 'entidad'));
  END IF;
  IF base IS NULL THEN
    base := 'entidad';
  END IF;
  IF char_length(base) < 2 THEN
    base := public.fn_normalizar_slug(base || '-1');
  END IF;
  IF public.fn_slug_reservado(base) THEN
    base := public.fn_slug_con_sufijo(base, 1);
  END IF;
  -- Un nombre con forma de UUID nunca debe confundirse con una clave interna.
  IF base ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' THEN
    base := base || '-x';
  END IF;
  RETURN left(base, 80);
END;
$$;

-- ── identity.empresas ────────────────────────────────────────────────────
ALTER TABLE identity.empresas ADD COLUMN IF NOT EXISTS slug text;

CREATE TABLE IF NOT EXISTS identity.empresa_slugs (
  slug           text PRIMARY KEY
                 CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$' AND char_length(slug) BETWEEN 2 AND 80),
  empresa_id     uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  reemplazado_en timestamptz
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_empresa_slugs_actual
  ON identity.empresa_slugs (empresa_id) WHERE reemplazado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_empresa_slugs_empresa
  ON identity.empresa_slugs (empresa_id, creado_en DESC);

-- El historial se mantiene por trigger para que ninguna via de escritura
-- (API, consola, backfill) pueda dejar slugs sin registro.
CREATE OR REPLACE FUNCTION identity.fn_empresa_slug_historial()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = identity, pg_temp
AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    INSERT INTO identity.empresa_slugs (empresa_id, slug) VALUES (NEW.id, NEW.slug);
  ELSIF NEW.slug IS DISTINCT FROM OLD.slug THEN
    UPDATE identity.empresa_slugs
       SET reemplazado_en = NOW()
     WHERE empresa_id = NEW.id AND reemplazado_en IS NULL;
    INSERT INTO identity.empresa_slugs (empresa_id, slug) VALUES (NEW.id, NEW.slug);
  END IF;
  RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS trg_empresa_slug_historial ON identity.empresas;
CREATE TRIGGER trg_empresa_slug_historial
AFTER INSERT OR UPDATE OF slug ON identity.empresas
FOR EACH ROW EXECUTE FUNCTION identity.fn_empresa_slug_historial();

-- Backfill determinista de empresas existentes (incluye las creadas antes de
-- ADR-26). Recorre una a una para resolver colisiones con sufijo -2, -3, ...
DO $$
DECLARE
  fila record;
  base text;
  candidato text;
  n integer;
BEGIN
  FOR fila IN
    SELECT id, nombre FROM identity.empresas
    WHERE slug IS NULL OR slug = '' OR public.fn_slug_reservado(slug)
  LOOP
    base := public.fn_slug_desde_nombre(fila.nombre, 'empresa');
    candidato := base;
    n := 2;
    WHILE EXISTS (
      SELECT 1 FROM identity.empresas e WHERE e.slug = candidato AND e.id <> fila.id
      UNION ALL
      SELECT 1 FROM identity.empresa_slugs s WHERE s.slug = candidato
    ) LOOP
      candidato := public.fn_slug_con_sufijo(base, n);
      n := n + 1;
    END LOOP;
    UPDATE identity.empresas SET slug = candidato WHERE id = fila.id;
  END LOOP;
END;
$$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_empresas_slug ON identity.empresas (slug);
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'empresas_slug_formato') THEN
    ALTER TABLE identity.empresas ADD CONSTRAINT empresas_slug_formato
      CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$' AND char_length(slug) BETWEEN 2 AND 80);
  END IF;
END;
$$;
ALTER TABLE identity.empresas ALTER COLUMN slug SET NOT NULL;

-- Garantiza historial para filas que ya tuvieran slug antes del trigger.
INSERT INTO identity.empresa_slugs (empresa_id, slug)
SELECT e.id, e.slug
FROM identity.empresas e
WHERE NOT EXISTS (
  SELECT 1 FROM identity.empresa_slugs s
  WHERE s.empresa_id = e.id AND s.reemplazado_en IS NULL
);

-- ── stores.tiendas ───────────────────────────────────────────────────────
ALTER TABLE stores.tiendas ADD COLUMN IF NOT EXISTS slug text;

CREATE TABLE IF NOT EXISTS stores.tienda_slugs (
  slug           text PRIMARY KEY
                 CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$' AND char_length(slug) BETWEEN 2 AND 80),
  tienda_id      uuid NOT NULL REFERENCES stores.tiendas(id) ON DELETE CASCADE,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  reemplazado_en timestamptz
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tienda_slugs_actual
  ON stores.tienda_slugs (tienda_id) WHERE reemplazado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_tienda_slugs_tienda
  ON stores.tienda_slugs (tienda_id, creado_en DESC);

CREATE OR REPLACE FUNCTION stores.fn_tienda_slug_historial()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = stores, pg_temp
AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    INSERT INTO stores.tienda_slugs (tienda_id, slug) VALUES (NEW.id, NEW.slug);
  ELSIF NEW.slug IS DISTINCT FROM OLD.slug THEN
    UPDATE stores.tienda_slugs
       SET reemplazado_en = NOW()
     WHERE tienda_id = NEW.id AND reemplazado_en IS NULL;
    INSERT INTO stores.tienda_slugs (tienda_id, slug) VALUES (NEW.id, NEW.slug);
  END IF;
  RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS trg_tienda_slug_historial ON stores.tiendas;
CREATE TRIGGER trg_tienda_slug_historial
AFTER INSERT OR UPDATE OF slug ON stores.tiendas
FOR EACH ROW EXECUTE FUNCTION stores.fn_tienda_slug_historial();

DO $$
DECLARE
  fila record;
  base text;
  candidato text;
  n integer;
BEGIN
  FOR fila IN
    SELECT id, nombre FROM stores.tiendas
    WHERE slug IS NULL OR slug = '' OR public.fn_slug_reservado(slug)
  LOOP
    base := public.fn_slug_desde_nombre(fila.nombre, 'tienda');
    candidato := base;
    n := 2;
    WHILE EXISTS (
      SELECT 1 FROM stores.tiendas t WHERE t.slug = candidato AND t.id <> fila.id
      UNION ALL
      SELECT 1 FROM stores.tienda_slugs s WHERE s.slug = candidato
    ) LOOP
      candidato := public.fn_slug_con_sufijo(base, n);
      n := n + 1;
    END LOOP;
    UPDATE stores.tiendas SET slug = candidato WHERE id = fila.id;
  END LOOP;
END;
$$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_tiendas_slug ON stores.tiendas (slug);
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'tiendas_slug_formato') THEN
    ALTER TABLE stores.tiendas ADD CONSTRAINT tiendas_slug_formato
      CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$' AND char_length(slug) BETWEEN 2 AND 80);
  END IF;
END;
$$;
ALTER TABLE stores.tiendas ALTER COLUMN slug SET NOT NULL;

INSERT INTO stores.tienda_slugs (tienda_id, slug)
SELECT t.id, t.slug
FROM stores.tiendas t
WHERE NOT EXISTS (
  SELECT 1 FROM stores.tienda_slugs s
  WHERE s.tienda_id = t.id AND s.reemplazado_en IS NULL
);

COMMIT;
