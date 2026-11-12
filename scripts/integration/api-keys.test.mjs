import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * API keys de integración (ADR-0005/R-GW-8) contra PostgreSQL real: alta,
 * listado, intercambio por JWT, revocación y rechazo posterior.
 */
const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

const MIGRACIONES = [
  '01_esquemas.sql',
  '01c_cqrs_catalog.sql',
  '01e_adr25_empresas.sql',
  '01f_slugs.sql',
  '01h_catalogo_por_empresa.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
  '62_api_keys.sql',
];

let client;
let pgService;
let apiKeys;
let empresaId;
let usuarioId;

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  try {
    await client.query('SELECT pg_advisory_lock(38194201)');
    const { rows: [{ existe }] } = await client.query(
      "SELECT to_regnamespace('identity') IS NOT NULL AS existe",
    );
    if (!existe) {
      for (const name of MIGRACIONES) {
        const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
        await client.query(sql);
      }
    } else {
      const sql = await readFile(new URL('../../infra/db/init/62_api_keys.sql', import.meta.url), 'utf8');
      await client.query(sql);
    }

    usuarioId = randomUUID();
    empresaId = randomUUID();
    const sufijo = randomUUID().slice(0, 8);
    await client.query(
      `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
       VALUES ($1, 'Integración API', $2, 'hash', 'empresario')`,
      [usuarioId, `api-${sufijo}@test.dev`],
    );
    await client.query(
      `INSERT INTO identity.empresas
         (id, nombre, slug, modalidad_logistica, vertical_id, pack_codigo, pack_version, creado_por)
       VALUES ($1, $2, $3, NULL, 'retail', 'pack_retail', 1, $4)`,
      [empresaId, `API ${sufijo}`, `api-${sufijo}`, usuarioId],
    );
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const shared = await import('@core/shared');
  const { ApiKeysService } = await import(
    '../../packages/identity-service/dist/api-keys/api-keys.service.js'
  );
  pgService = new shared.PgService({ url: connectionString });
  await pgService.onModuleInit();
  apiKeys = new ApiKeysService(pgService);
});

after(async () => {
  await client.query('DELETE FROM identity.api_keys WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await client.query('DELETE FROM identity.empresas WHERE id = $1', [empresaId]).catch(() => undefined);
  await client.query('DELETE FROM identity.usuarios WHERE id = $1', [usuarioId]).catch(() => undefined);
  await pgService?.onModuleDestroy();
  await client?.end();
});

const actor = () => ({
  user_id: usuarioId,
  email: 'api@test.dev',
  rol: 'empresario',
  tenant_id: empresaId,
});

test('crea, lista, intercambia y revoca una API key', async () => {
  const creada = await apiKeys.crear(actor(), {
    nombre: 'Contabilidad',
    rol: 'vendedor',
    expira_en_dias: 30,
  });
  assert.match(creada.key, /^cek_[0-9a-f]{8}_/);
  assert.equal(creada.estado, 'activa');
  assert.equal(creada.rol, 'vendedor');

  const listado = await apiKeys.listar(actor());
  assert.equal(listado.length, 1);
  assert.equal(listado[0].prefijo, creada.prefijo);
  assert.equal(listado[0].hash, undefined, 'el hash no debe salir en el listado');

  const token = await apiKeys.token(creada.key);
  assert.equal(token.token_type, 'Bearer');
  assert.equal(token.expires_in, 900);

  const shared = await import('@core/shared');
  const payload = shared.verificarToken(token.access_token, shared.secretoJwt());
  assert.equal(payload.rol, 'vendedor');
  assert.equal(payload.tenant_id, empresaId);
  assert.equal(payload.sub, `api-key:${creada.id}`);

  await apiKeys.revocar(actor(), creada.id);
  await assert.rejects(
    apiKeys.token(creada.key),
    (error) => error?.codigo === 'NO_AUTORIZADO',
  );
});

test('rechaza keys inexistentes y roles administrativos', async () => {
  await assert.rejects(
    apiKeys.token('cek_00000000_noexiste'),
    (error) => error?.codigo === 'NO_AUTORIZADO',
  );
  await assert.rejects(
    apiKeys.crear(actor(), { nombre: 'root', rol: 'super_admin' }),
    (error) => error?.codigo === 'ROL_NO_PERMITIDO',
  );
});
