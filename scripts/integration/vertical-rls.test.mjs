import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * RLS multi-vertical (R-DB-2/R-SB-3/R-I-12).
 *
 * Ejecuta consultas con el rol de aplicación y claims JWT de dos empresas de
 * verticales distintas. Comprueba lectura, escritura y actualización cruzada;
 * el admin global conserva visibilidad.
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

process.env.DATABASE_URL = connectionString;
process.env.STORAGE_SERVICE_URL = 'http://127.0.0.1:9';

const MIGRACIONES_LEGACY = [
  '01_esquemas.sql',
  '01c_cqrs_catalog.sql',
  '01e_adr25_empresas.sql',
  '01f_slugs.sql',
  '01h_catalogo_por_empresa.sql',
  '01i_variantes_imagenes.sql',
  '11_catalogo_productos_servicios.sql',
  '25_catalog_items.sql',
  '13_empresa_metadatos.sql',
  '16_media_background_plataforma.sql',
  '17_media_background_empresa.sql',
  '19_personalizacion_tienda.sql',
  '22_herencia_tema.sql',
  '28_scheduling.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
  '30_pack_backfill.sql',
  '31_multi_vertical_rls.sql',
  '37_personal_empresa.sql',
];

const MIGRACIONES_FINAL = [
  '13_empresa_metadatos.sql',
  '16_media_background_plataforma.sql',
  '17_media_background_empresa.sql',
  '19_personalizacion_tienda.sql',
  '22_herencia_tema.sql',
  '24_multi_vertical.sql',
  '28_scheduling.sql',
  '29_vertical_packs.sql',
  '30_pack_backfill.sql',
  '31_multi_vertical_rls.sql',
  '37_personal_empresa.sql',
];

let client;
let pgService;
let empresas;
const tenants = {};

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    const { rows: [{ existe }] } = await client.query(
      `SELECT to_regnamespace('identity') IS NOT NULL AS existe`,
    );
    const { rows: [{ final }] } = await client.query(
      `SELECT to_regclass('catalog.items') IS NOT NULL
              AND to_regclass('catalog.productos') IS NULL AS final`,
    );
    const migraciones = final
      ? MIGRACIONES_FINAL
      : existe
        ? MIGRACIONES_LEGACY.filter((name) => name !== '01_esquemas.sql')
        : MIGRACIONES_LEGACY;
    if (final) {
      // 31 todavía declara una policy sobre la vista legacy retirada por 48.
      // El stub permite aplicar la policy de las tablas finales sin resucitar
      // el catálogo legacy.
      await client.query(
        `CREATE TABLE IF NOT EXISTS catalog.producto_catalogo_vista
           (id uuid, empresa_id uuid, estado_operativo text)`,
      );
    }
    for (const name of migraciones) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
    if (final) {
      await client.query('DROP TABLE IF EXISTS catalog.producto_catalogo_vista');
    }
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const shared = await import('@core/shared');
  const { PgService } = shared;
  const { EmpresasService } = await import(
    '../../packages/identity-service/dist/empresas/empresas.service.js'
  );
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
  empresas = new EmpresasService(pgService, {});

  tenants.retail = await crearTenant('retail', 'pack_retail');
  tenants.belleza = await crearTenant('belleza', 'pack_belleza');

  tenants.retail.itemId = await crearItem(tenants.retail.empresaId, 'RETAIL');
  tenants.belleza.itemId = await crearItem(tenants.belleza.empresaId, 'BELLEZA');
  tenants.retail.recursoId = await crearRecurso(tenants.retail.empresaId, 'Retail');
  tenants.belleza.recursoId = await crearRecurso(tenants.belleza.empresaId, 'Belleza');
  tenants.retail.citaId = await crearCita(tenants.retail, '2032-05-10T14:00:00Z');
  tenants.belleza.citaId = await crearCita(tenants.belleza, '2032-05-10T15:00:00Z');
});

after(async () => {
  for (const tenant of Object.values(tenants)) {
    await client.query('DELETE FROM scheduling.citas WHERE empresa_id = $1', [tenant.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM scheduling.recursos WHERE empresa_id = $1', [tenant.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM catalog.items WHERE empresa_id = $1', [tenant.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.empresa_capacidades WHERE empresa_id = $1', [tenant.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.asignaciones_personal WHERE empresa_id = $1', [tenant.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.roles_empresa WHERE empresa_id = $1', [tenant.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.empresas WHERE id = $1', [tenant.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.usuarios WHERE id = $1', [tenant.usuarioId]).catch(() => undefined);
  }
  await pgService?.onModuleDestroy();
  await client?.end();
});

async function crearTenant(vertical, packCodigo) {
  const usuarioId = randomUUID();
  await client.query(
    `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
     VALUES ($1, 'Empresario RLS', $2, 'hash', 'empresario')`,
    [usuarioId, `rls-${usuarioId}@test.dev`],
  );
  const empresa = await empresas.crear(
    usuarioId,
    `Tenant RLS ${vertical} ${randomUUID().slice(0, 8)}`,
    undefined,
    undefined,
    vertical,
    packCodigo,
  );
  return { empresaId: empresa.id, usuarioId, vertical };
}

async function crearItem(empresaId, nombre) {
  const { rows: [item] } = await client.query(
    `INSERT INTO catalog.items
       (empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock)
     VALUES ($1, 'producto', $2, $3, 1000, true, 5)
     RETURNING id`,
    [empresaId, `RLS-${randomUUID().slice(0, 8)}`, `Item ${nombre}`],
  );
  return item.id;
}

async function crearRecurso(empresaId, nombre) {
  const { rows: [recurso] } = await client.query(
    `INSERT INTO scheduling.recursos (empresa_id, tipo, nombre)
     VALUES ($1, 'profesional', $2) RETURNING id`,
    [empresaId, `Recurso ${nombre}`],
  );
  return recurso.id;
}

async function crearCita(tenant, inicia) {
  const termina = new Date(new Date(inicia).getTime() + 60 * 60 * 1000).toISOString();
  const { rows: [cita] } = await client.query(
    `INSERT INTO scheduling.citas
       (empresa_id, recurso_id, inicia_en, termina_en, notas)
     VALUES ($1, $2, $3, $4, 'Cita RLS') RETURNING id`,
    [tenant.empresaId, tenant.recursoId, inicia, termina],
  );
  return cita.id;
}

async function conClaims(tenant, fn) {
  await client.query('BEGIN');
  try {
    await client.query('SET LOCAL ROLE core_engine_app');
    await client.query(
      `SELECT set_config('request.jwt.claims', $1, true)`,
      [JSON.stringify({
        sub: tenant.usuarioId,
        rol: 'empresario',
        tenant_id: tenant.empresaId,
      })],
    );
    return await fn();
  } finally {
    await client.query('ROLLBACK');
  }
}

test('un empresario solo ve sus items, capacidades y citas', async () => {
  const retail = await conClaims(tenants.retail, async () => {
    const items = await client.query('SELECT id, empresa_id FROM catalog.items ORDER BY id');
    const capacidades = await client.query(
      'SELECT empresa_id, capacidad_codigo FROM identity.empresa_capacidades ORDER BY capacidad_codigo',
    );
    const citas = await client.query('SELECT id, empresa_id FROM scheduling.citas ORDER BY id');
    return { items: items.rows, capacidades: capacidades.rows, citas: citas.rows };
  });

  assert.deepEqual(retail.items.map((fila) => fila.id), [tenants.retail.itemId]);
  assert.deepEqual(retail.citas.map((fila) => fila.id), [tenants.retail.citaId]);
  assert.ok(retail.capacidades.length > 0);
  assert.ok(retail.capacidades.every((fila) => fila.empresa_id === tenants.retail.empresaId));

  const belleza = await conClaims(tenants.belleza, async () => {
    const items = await client.query('SELECT id FROM catalog.items ORDER BY id');
    return items.rows.map((fila) => fila.id);
  });
  assert.deepEqual(belleza, [tenants.belleza.itemId]);
});

test('un tenant no puede insertar un item de otra empresa', async () => {
  await assert.rejects(
    conClaims(tenants.retail, () =>
      client.query(
        `INSERT INTO catalog.items
           (empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock)
         VALUES ($1, 'producto', $2, 'Intruso', 1000, true, 1)`,
        [tenants.belleza.empresaId, `RLS-X-${randomUUID().slice(0, 8)}`],
      ),
    ),
    (error) => error?.code === '42501',
  );
});

test('un tenant no puede actualizar ni borrar datos de otra empresa', async () => {
  const actualizadas = await conClaims(tenants.retail, () =>
    client.query('UPDATE catalog.items SET nombre = $1 WHERE id = $2', [
      'Secuestrado',
      tenants.belleza.itemId,
    ]),
  );
  assert.equal(actualizadas.rowCount, 0);

  const borradas = await conClaims(tenants.retail, () =>
    client.query('DELETE FROM scheduling.citas WHERE id = $1', [tenants.belleza.citaId]),
  );
  assert.equal(borradas.rowCount, 0);
});

test('el admin global conserva visibilidad de ambas verticales', async () => {
  await client.query('BEGIN');
  try {
    await client.query('SET LOCAL ROLE core_engine_app');
    await client.query(
      `SELECT set_config('request.jwt.claims', $1, true)`,
      [JSON.stringify({ sub: randomUUID(), rol: 'super_admin' })],
    );
    const items = await client.query('SELECT empresa_id FROM catalog.items ORDER BY empresa_id');
    const empresas = new Set(items.rows.map((fila) => fila.empresa_id));
    assert.ok(empresas.has(tenants.retail.empresaId));
    assert.ok(empresas.has(tenants.belleza.empresaId));
  } finally {
    await client.query('ROLLBACK');
  }
});
