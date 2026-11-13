import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Limpieza de combos (Fase P1.5): catalog.combos se retira y los combos pasan
 * a ser items con `es_combo=true` + componentes en catalog.item_componentes.
 * La migracion conserva el id del combo para no romper clientes.
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

process.env.DATABASE_URL = connectionString;

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
  '60_catalog_item_componentes.sql',
];

let client;
let pgService;
let operaciones;
const empresaId = randomUUID();
const componenteId = randomUUID();
const comboId = randomUUID();
let habiaCombos = false;

async function aplicar(nombre) {
  const sql = await readFile(new URL(`../../infra/db/init/${nombre}`, import.meta.url), 'utf8');
  await client.query(sql);
}

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    await client.query(`
      CREATE SCHEMA IF NOT EXISTS auth;
      CREATE OR REPLACE FUNCTION auth.jwt() RETURNS jsonb LANGUAGE sql STABLE AS $$
        SELECT COALESCE(NULLIF(current_setting('request.jwt.claims', true), ''), '{}')::jsonb
      $$;
      CREATE OR REPLACE FUNCTION auth.rol() RETURNS text LANGUAGE sql STABLE AS $$
        SELECT COALESCE(auth.jwt() ->> 'rol', 'anon')
      $$;
      DO $$ BEGIN
        IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN CREATE ROLE anon NOLOGIN; END IF;
        IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN CREATE ROLE authenticated NOLOGIN; END IF;
      END $$;
    `);
    const { rows: [{ final }] } = await client.query(
      `SELECT to_regclass('catalog.items') IS NOT NULL
              AND to_regclass('catalog.productos') IS NULL AS final`,
    );
    if (!final) {
      const { rows: [{ existe }] } = await client.query(
        `SELECT to_regnamespace('identity') IS NOT NULL AS existe`,
      );
      const migraciones = existe
        ? MIGRACIONES.filter((nombre) => nombre !== '01_esquemas.sql')
        : MIGRACIONES;
      for (const nombre of migraciones) await aplicar(nombre);
    }
    await aplicar('60_catalog_item_componentes.sql');

    await client.query(
      `INSERT INTO catalog.items
         (id, empresa_id, tipo, codigo, nombre, precio_cents, costo_cents,
          track_inventory, stock)
       VALUES ($1, $2, 'producto', 'COMP-01', 'Componente', 1000, 500, true, 10)`,
      [componenteId, empresaId],
    );

    // Solo existe si la migracion 68 aun no se aplico.
    const { rows: [{ existe: hayCombos }] } = await client.query(
      `SELECT to_regclass('catalog.combos') IS NOT NULL AS existe`,
    );
    habiaCombos = hayCombos;
    if (hayCombos) {
      await client.query(
        `INSERT INTO catalog.combos
           (id, empresa_id, sku, nombre, descripcion, codigo_barras, precio_cents,
            estado, componentes)
         VALUES ($1, $2, 'COMBO-01', 'Combo legado', 'Migrado a BOM', '750001', 3000,
                 'activo', $3::jsonb)`,
        [
          comboId,
          empresaId,
          JSON.stringify([{
            item_id: componenteId,
            sku: 'COMP-01',
            nombre: 'Componente',
            cantidad: 2,
          }]),
        ],
      );
    }
    await aplicar('68_retirar_combos.sql');
  } catch (error) {
    await client.query('ROLLBACK').catch(() => undefined);
    console.error('combos-bom setup:', error);
    throw error;
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const [{ PgService }, { OperacionesService }, { ProductosService }] = await Promise.all([
    import('@core/shared'),
    import('../../packages/catalog-service/dist/operaciones/operaciones.service.js'),
    import('../../packages/catalog-service/dist/productos/productos.service.js'),
  ]);
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
  operaciones = new OperacionesService(
    pgService,
    new ProductosService(pgService),
    {},
  );
});

after(async () => {
  await client.query('DELETE FROM catalog.items WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await pgService?.onModuleDestroy();
  await client?.end();
});

test('migra combos a items + BOM y mantiene el contrato de la API', { concurrency: false }, async () => {
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    const { rows: [{ existe }] } = await client.query(
      `SELECT to_regclass('catalog.combos') IS NOT NULL AS existe`,
    );
    assert.equal(existe, false, 'catalog.combos debe haberse retirado');

    if (habiaCombos) {
      const legado = await operaciones.obtenerCombo(comboId, empresaId);
      assert.ok(legado);
      assert.equal(legado.sku, 'COMBO-01');
      assert.equal(legado.costo_total, '10.00');
      assert.equal(legado.componentes.length, 1);
      assert.equal(legado.componentes[0].item_id, componenteId);

      const pagina = await operaciones.listarCombos(empresaId, { q: 'combo' });
      assert.equal(pagina.total, 1);
    }

    const creado = await operaciones.crearCombo(empresaId, {
      sku: 'COMBO-02',
      nombre: 'Combo nuevo',
      precio: '25.00',
      componentes: [{ item_id: componenteId, sku: 'COMP-01', cantidad: 1 }],
    });
    assert.equal(creado.sku, 'COMBO-02');
    assert.equal(creado.costo_total, '5.00');

    const actualizado = await operaciones.actualizarCombo(creado.id, empresaId, {
      precio: '30.00',
      componentes: [{ item_id: componenteId, sku: 'COMP-01', cantidad: 3 }],
    });
    assert.equal(actualizado.precio, '30.00');
    assert.equal(actualizado.costo_total, '15.00');

    await assert.rejects(
      operaciones.crearCombo(empresaId, {
        sku: 'COMBO-03',
        nombre: 'Combo anidado',
        precio: '10.00',
        componentes: [{ item_id: creado.id, sku: creado.sku, cantidad: 1 }],
      }),
      (error) => error?.codigo === 'COMPONENTE_INVALIDO',
    );

    await operaciones.eliminarCombo(creado.id, empresaId);
    assert.equal(await operaciones.obtenerCombo(creado.id, empresaId), null);

    const { rows } = await client.query(
      `SELECT es_combo, eliminado_en FROM catalog.items WHERE id = $1`,
      [creado.id],
    );
    assert.equal(rows[0].es_combo, true);
    assert.notEqual(rows[0].eliminado_en, null);
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
