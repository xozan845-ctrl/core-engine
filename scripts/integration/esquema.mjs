/**
 * Esquema final para las suites de integración (ADR-0018).
 *
 * Aplica la cadena mínima que crea `item`, `pricing` e `inventory` y retira
 * `catalog`. Evita las migraciones de PostGIS porque el job de contratos usa
 * `postgres:16-alpine`. El preludio replica los roles y helpers `auth.*` que
 * necesitan las políticas RLS de las migraciones intermedias.
 */
import { readFile } from 'node:fs/promises';

const MIGRACIONES = [
  '01_esquemas.sql',
  '01c_cqrs_catalog.sql',
  '01e_adr25_empresas.sql',
  '01f_slugs.sql',
  '01h_catalogo_por_empresa.sql',
  '01i_variantes_imagenes.sql',
  '11_catalogo_productos_servicios.sql',
  '12_catalogo_operaciones_avanzadas.sql',
  '25_catalog_items.sql',
  '26_stores_items.sql',
  '42a_catalog_items_vista_reset.sql',
  '43_catalog_item_model.sql',
  '44_catalog_items_metadatos.sql',
  '48_catalog_retiro_legacy.sql',
  '53_reservas_lotes.sql',
  '59_catalog_item_tarifas.sql',
  '60_catalog_item_componentes.sql',
  '64_listas_precio_escalas.sql',
  '65_inventario_ubicaciones.sql',
  '66_item_series.sql',
  '75_inventory_schema.sql',
  '79_pricing_schema.sql',
  '81_item_schema.sql',
  '83_item_fk_externas.sql',
  '84_inventory_stock_owner.sql',
  '85_retiro_catalog.sql',
];

const PRELUDIO = `
CREATE SCHEMA IF NOT EXISTS auth;
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
    CREATE ROLE anon NOLOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
    CREATE ROLE authenticated NOLOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    CREATE ROLE core_engine_app NOLOGIN;
  END IF;
END $$;
CREATE OR REPLACE FUNCTION auth.jwt() RETURNS jsonb
  LANGUAGE sql STABLE
  AS $fn$ SELECT COALESCE(NULLIF(current_setting('request.jwt.claims', true), ''), '{}')::jsonb $fn$;
CREATE OR REPLACE FUNCTION auth.uid() RETURNS uuid
  LANGUAGE sql STABLE
  AS $fn$ SELECT NULLIF(auth.jwt() ->> 'sub', '')::uuid $fn$;
CREATE OR REPLACE FUNCTION auth.rol() RETURNS text
  LANGUAGE sql STABLE
  AS $fn$ SELECT COALESCE(auth.jwt() ->> 'rol', 'anon') $fn$;
`;

/** Deja la base con el esquema final (item/pricing/inventory, sin catalog). */
export async function asegurarEsquemaFinal(client) {
  const { rows: [{ listo }] } = await client.query(
    `SELECT to_regclass('item.items') IS NOT NULL
            AND to_regnamespace('catalog') IS NULL AS listo`,
  );
  if (listo) return;

  await client.query(PRELUDIO);
  for (const nombre of MIGRACIONES) {
    const sql = await readFile(
      new URL(`../../infra/db/init/${nombre}`, import.meta.url),
      'utf8',
    );
    await client.query(sql);
  }
}
