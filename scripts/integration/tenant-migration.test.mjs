import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

const MIGRACIONES = [
  '01_esquemas.sql',
  '01e_adr25_empresas.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
  '30_pack_backfill.sql',
];
let client;

async function aplicarMigraciones() {
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
}

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    await aplicarMigraciones();
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});

after(async () => {
  await client?.end();
});

test('el backfill asigna el pack de la vertical y deja auditoria', { concurrency: false }, async () => {
  const usuarioId = randomUUID();
  const empresaId = randomUUID();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    await client.query('BEGIN');
    await client.query(
      `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
       VALUES ($1, 'Migracion', $2, 'hash', 'empresario')`,
      [usuarioId, `${usuarioId}@migration.test`],
    );
    await client.query(
      `INSERT INTO identity.empresas
         (id, nombre, slug, vertical_id, pack_codigo, pack_version, creado_por)
       VALUES ($1, 'Tenant legado', $3, 'retail', NULL, NULL, $2)`,
      [empresaId, usuarioId, `tenant-${empresaId.slice(0, 8)}`],
    );
    await client.query('COMMIT');

    await client.query(await readFile(new URL('../../infra/db/init/30_pack_backfill.sql', import.meta.url), 'utf8'));
    const { rows: [empresa] } = await client.query(
      `SELECT pack_codigo, pack_version
         FROM identity.empresas
        WHERE id = $1`,
      [empresaId],
    );
    const { rows: [auditoria] } = await client.query(
      `SELECT estado, pack_codigo, capacidades_antes, capacidades_despues
         FROM identity.pack_migraciones
        WHERE empresa_id = $1`,
      [empresaId],
    );
    assert.deepEqual(empresa, { pack_codigo: 'pack_retail', pack_version: 1 });
    assert.equal(auditoria.estado, 'aplicado');
    assert.equal(auditoria.pack_codigo, 'pack_retail');
    assert.ok(auditoria.capacidades_despues >= auditoria.capacidades_antes);
  } finally {
    await client.query('ROLLBACK').catch(() => {});
    await client.query('BEGIN');
    await client.query('DELETE FROM identity.pack_migraciones WHERE empresa_id = $1', [empresaId]);
    await client.query('DELETE FROM identity.empresa_capacidades WHERE empresa_id = $1', [empresaId]);
    await client.query('DELETE FROM identity.empresas WHERE id = $1', [empresaId]);
    await client.query('DELETE FROM identity.usuarios WHERE id = $1', [usuarioId]);
    await client.query('COMMIT');
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
