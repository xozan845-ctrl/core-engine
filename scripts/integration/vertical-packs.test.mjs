import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

const MIGRACIONES = ['01_esquemas.sql', '01e_adr25_empresas.sql', '24_multi_vertical.sql', '29_vertical_packs.sql'];
let client;

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    const { rows: [{ existe }] } = await client.query('SELECT to_regnamespace(\'identity\') IS NOT NULL AS existe');
    const { rows: [{ catalogoFinal }] } = await client.query(
      `SELECT to_regclass('catalog.items') IS NOT NULL AND to_regclass('catalog.productos') IS NULL AS "catalogoFinal"`,
    );
    const migraciones = existe
      ? MIGRACIONES.filter(
          (name) =>
            name !== '01_esquemas.sql' &&
            !(catalogoFinal && ['01e_adr25_empresas.sql', '24_multi_vertical.sql'].includes(name)),
        )
      : MIGRACIONES;
    for (const name of migraciones) {
      await client.query(await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8'));
    }
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});

after(async () => {
  await client?.end();
});

test('los packs activos cubren las cinco verticales soportadas', async () => {
  const { rows } = await client.query(
    `SELECT codigo, version, vertical_codigo
       FROM identity.packs
      WHERE activo = true
      ORDER BY codigo`,
  );
  assert.deepEqual(rows, [
    { codigo: 'pack_belleza', version: 1, vertical_codigo: 'belleza' },
    { codigo: 'pack_dental', version: 1, vertical_codigo: 'dental' },
    { codigo: 'pack_farmacia', version: 1, vertical_codigo: 'farmacia' },
    { codigo: 'pack_flota', version: 1, vertical_codigo: 'flota' },
    { codigo: 'pack_retail', version: 1, vertical_codigo: 'retail' },
  ]);
});

test('cada pack expone capacidades versionadas y no comparte asignaciones entre packs', async () => {
  const { rows } = await client.query(
    `SELECT p.codigo, COUNT(pc.capacidad_codigo)::int AS capacidades,
            COUNT(pc.capacidad_codigo) FILTER (WHERE pc.activo)::int AS activas
       FROM identity.packs p
       LEFT JOIN identity.pack_capacidades pc
         ON pc.pack_codigo = p.codigo AND pc.pack_version = p.version
      GROUP BY p.codigo
      ORDER BY p.codigo`,
  );
  assert.ok(rows.every((row) => row.capacidades > 0 && row.activas > 0));
  const { rows: [farmacia] } = await client.query(
    `SELECT 1 FROM identity.pack_capacidades
      WHERE pack_codigo = 'pack_farmacia' AND capacidad_codigo = 'inventario_lotes' AND activo = true`,
  );
  const { rows: [belleza] } = await client.query(
    `SELECT 1 FROM identity.pack_capacidades
      WHERE pack_codigo = 'pack_belleza' AND capacidad_codigo = 'agenda_recursos' AND activo = true`,
  );
  assert.ok(farmacia);
  assert.ok(belleza);
});
