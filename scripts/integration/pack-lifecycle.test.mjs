import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Ciclo de vida de packs (ADR-0001/R-I-12).
 *
 * Crea una vertical, un modelo y dos versiones de pack contra PostgreSQL real;
 * comprueba que un tenant nuevo hereda la versión elegida y que las versiones
 * anteriores no se reescriben.
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

process.env.DATABASE_URL = connectionString;
process.env.STORAGE_SERVICE_URL = 'http://127.0.0.1:9';

const MIGRACIONES_IDENTITY = [
  '01_esquemas.sql',
  '01c_cqrs_catalog.sql',
  '01e_adr25_empresas.sql',
  '01f_slugs.sql',
  '01h_catalogo_por_empresa.sql',
  '13_empresa_metadatos.sql',
  '16_media_background_plataforma.sql',
  '17_media_background_empresa.sql',
  '19_personalizacion_tienda.sql',
  '22_herencia_tema.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
  '30_pack_backfill.sql',
  '32_pack_builder.sql',
  '37_personal_empresa.sql',
];

const MIGRACIONES_FINAL = [
  '13_empresa_metadatos.sql',
  '16_media_background_plataforma.sql',
  '17_media_background_empresa.sql',
  '19_personalizacion_tienda.sql',
  '22_herencia_tema.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
  '30_pack_backfill.sql',
  '32_pack_builder.sql',
  '37_personal_empresa.sql',
];

let client;
let pgService;
let packs;
let empresas;
const creados = [];

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
        ? MIGRACIONES_IDENTITY.filter((name) => name !== '01_esquemas.sql')
        : MIGRACIONES_IDENTITY;
    // 32_pack_builder declara políticas contra auth.rol() y los roles de aplicación.
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
        IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN CREATE ROLE core_engine_app NOLOGIN; END IF;
      END $$;
    `);
    for (const name of migraciones) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const shared = await import('@core/shared');
  const { PgService } = shared;
  const [{ PacksService }, { EmpresasService }] = await Promise.all([
    import('../../packages/identity-service/dist/packs/packs.service.js'),
    import('../../packages/identity-service/dist/empresas/empresas.service.js'),
  ]);
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
  packs = new PacksService(pgService);
  empresas = new EmpresasService(pgService, {});
});

after(async () => {
  for (const creado of creados.reverse()) {
    await limpiarTenant(creado).catch(() => undefined);
    if (creado.packCodigo) {
      await client.query('DELETE FROM identity.packs WHERE codigo = $1', [creado.packCodigo]).catch(() => undefined);
    }
    if (creado.vertical) {
      await client.query('DELETE FROM identity.verticales WHERE codigo = $1', [creado.vertical]).catch(() => undefined);
    }
    if (creado.modelo) {
      await client.query('DELETE FROM identity.modelos_operacion WHERE codigo = $1', [creado.modelo]).catch(() => undefined);
    }
  }
  await pgService?.onModuleDestroy();
  await client?.end();
});

async function crearUsuario() {
  const usuarioId = randomUUID();
  await client.query(
    `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
     VALUES ($1, 'Empresario packs', $2, 'hash', 'empresario')`,
    [usuarioId, `packs-${usuarioId}@test.dev`],
  );
  return usuarioId;
}

async function limpiarTenant({ empresaId, usuarioId }) {
  if (empresaId) {
    await client.query('DELETE FROM identity.empresa_capacidades WHERE empresa_id = $1', [empresaId]);
    await client.query('DELETE FROM identity.asignaciones_personal WHERE empresa_id = $1', [empresaId]);
    await client.query('DELETE FROM identity.roles_empresa WHERE empresa_id = $1', [empresaId]);
    await client.query('DELETE FROM identity.empresas WHERE id = $1', [empresaId]);
  }
  if (usuarioId) await client.query('DELETE FROM identity.usuarios WHERE id = $1', [usuarioId]);
}

async function crearTenant({ vertical, packCodigo, sufijo }) {
  const usuarioId = await crearUsuario();
  const registro = { usuarioId, vertical, packCodigo };
  creados.push(registro);
  const empresa = await empresas.crear(
    usuarioId,
    `Tenant pack ${sufijo}`,
    undefined,
    undefined,
    vertical,
    packCodigo,
  );
  registro.empresaId = empresa.id;
  return empresa;
}

async function capacidadesActivas(empresaId) {
  const { rows } = await client.query(
    `SELECT capacidad_codigo
       FROM identity.empresa_capacidades
      WHERE empresa_id = $1 AND activo = true
      ORDER BY capacidad_codigo`,
    [empresaId],
  );
  return rows.map((fila) => fila.capacidad_codigo);
}

test('un pack v1 define la vertical, el modelo y las capacidades del tenant', async () => {
  const sufijo = randomUUID().slice(0, 8);
  const modelo = `modelo_${sufijo}`;
  const vertical = `mv_${sufijo}`;
  const packCodigo = `pack_mv_${sufijo}`;
  creados.push({ vertical, modelo, packCodigo });

  await packs.crearModelo({
    codigo: modelo,
    nombre: 'Servicios agendados',
    descripcion: 'Modelo de prueba',
    config: { requiere_agenda: true },
  });
  await packs.crearVertical({
    codigo: vertical,
    nombre: 'Vertical de prueba',
    modelo_operacion: modelo,
  });
  const pack = await packs.crear({
    codigo: packCodigo,
    version: 1,
    vertical_codigo: vertical,
    nombre: 'Pack de prueba v1',
    config: { canal: 'piloto' },
    modelos: [{ codigo: modelo }],
    capacidades: [{ codigo: 'catalogo_items' }, { codigo: 'agenda_recursos' }],
  });

  assert.equal(pack.vertical_codigo, vertical);
  assert.equal(pack.version, 1);
  assert.deepEqual(pack.capacidades.map((c) => c.capacidad_codigo).sort(), [
    'agenda_recursos',
    'catalogo_items',
  ]);
  assert.deepEqual(pack.modelos.map((m) => m.modelo_codigo), [modelo]);

  const empresa = await crearTenant({ vertical, packCodigo, sufijo });
  assert.equal(empresa.vertical_id, vertical);
  assert.equal(empresa.pack_codigo, packCodigo);
  assert.equal(empresa.pack_version, 1);
  assert.deepEqual(await capacidadesActivas(empresa.id), ['agenda_recursos', 'catalogo_items']);
});

test('un pack v2 no reescribe el tenant que quedó en v1', async () => {
  const sufijo = randomUUID().slice(0, 8);
  const modelo = `modelo_${sufijo}`;
  const vertical = `mv_${sufijo}`;
  const packCodigo = `pack_mv_${sufijo}`;
  creados.push({ vertical, modelo, packCodigo });

  await packs.crearModelo({ codigo: modelo, nombre: 'Modelo versionado' });
  await packs.crearVertical({ codigo: vertical, nombre: 'Vertical versionada', modelo_operacion: modelo });
  await packs.crear({
    codigo: packCodigo,
    version: 1,
    vertical_codigo: vertical,
    nombre: 'Pack versionado v1',
    capacidades: [{ codigo: 'catalogo_items' }, { codigo: 'agenda_recursos' }],
  });
  const tenantV1 = await crearTenant({ vertical, packCodigo, sufijo: `${sufijo}-v1` });

  const packV2 = await packs.crear({
    codigo: packCodigo,
    version: 2,
    vertical_codigo: vertical,
    nombre: 'Pack versionado v2',
    capacidades: [{ codigo: 'catalogo_items' }, { codigo: 'facturacion' }],
  });
  assert.equal(packV2.version, 2);
  assert.deepEqual(packV2.capacidades.map((c) => c.capacidad_codigo).sort(), [
    'catalogo_items',
    'facturacion',
  ]);

  const tenantV2 = await crearTenant({ vertical, packCodigo, sufijo: `${sufijo}-v2` });
  assert.equal(tenantV2.pack_version, 2);
  assert.deepEqual(await capacidadesActivas(tenantV2.id), ['catalogo_items', 'facturacion']);

  const v1Db = await client.query(
    `SELECT pack_version FROM identity.empresas WHERE id = $1`,
    [tenantV1.id],
  );
  assert.equal(v1Db.rows[0].pack_version, 1);
  assert.deepEqual(await capacidadesActivas(tenantV1.id), ['agenda_recursos', 'catalogo_items']);

  const listado = await packs.listar(vertical);
  assert.deepEqual(listado.map((p) => p.version).sort(), [1, 2]);
});

test('rechaza un pack con capacidad inexistente sin dejar el pack a medias', async () => {
  const sufijo = randomUUID().slice(0, 8);
  const modelo = `modelo_${sufijo}`;
  const vertical = `mv_${sufijo}`;
  const packCodigo = `pack_mv_${sufijo}`;
  creados.push({ vertical, modelo, packCodigo });

  await packs.crearModelo({ codigo: modelo, nombre: 'Modelo rechazo' });
  await packs.crearVertical({ codigo: vertical, nombre: 'Vertical rechazo', modelo_operacion: modelo });

  await assert.rejects(
    packs.crear({
      codigo: packCodigo,
      version: 1,
      vertical_codigo: vertical,
      nombre: 'Pack inválido',
      capacidades: [{ codigo: 'capacidad_que_no_existe' }],
    }),
    (error) => error?.codigo === 'NO_ENCONTRADO',
  );

  const { rows } = await client.query(
    `SELECT 1 FROM identity.packs WHERE codigo = $1 AND version = 1`,
    [packCodigo],
  );
  assert.equal(rows.length, 0, 'la transacción debe revertir el pack completo');
});

test('actualizarPack reconcilia capacidades y deja auditoria', async () => {
  const sufijo = randomUUID().slice(0, 8);
  const modelo = `modelo_${sufijo}`;
  const vertical = `mv_${sufijo}`;
  const packCodigo = `pack_mv_${sufijo}`;
  creados.push({ vertical, modelo, packCodigo });

  await packs.crearModelo({ codigo: modelo, nombre: 'Modelo upgrade' });
  await packs.crearVertical({ codigo: vertical, nombre: 'Vertical upgrade', modelo_operacion: modelo });
  await packs.crear({
    codigo: packCodigo,
    version: 1,
    vertical_codigo: vertical,
    nombre: 'Pack upgrade v1',
    capacidades: [{ codigo: 'catalogo_items' }, { codigo: 'agenda_recursos' }],
  });
  const empresa = await crearTenant({ vertical, packCodigo, sufijo });
  await packs.crear({
    codigo: packCodigo,
    version: 2,
    vertical_codigo: vertical,
    nombre: 'Pack upgrade v2',
    capacidades: [{ codigo: 'catalogo_items' }, { codigo: 'facturacion' }],
  });

  const actualizada = await empresas.actualizarPack(empresa.id, packCodigo, 2);
  assert.equal(actualizada.pack_version, 2);
  assert.deepEqual(await capacidadesActivas(empresa.id), ['catalogo_items', 'facturacion']);

  const { rows: [auditoria] } = await client.query(
    `SELECT pack_codigo, pack_version, detalles
       FROM identity.pack_migraciones
      WHERE empresa_id = $1`,
    [empresa.id],
  );
  assert.equal(auditoria.pack_codigo, packCodigo);
  assert.equal(auditoria.pack_version, 2);
  assert.equal(auditoria.detalles.origen, 'pack_upgrade');
  assert.equal(auditoria.detalles.version_anterior, 1);
  assert.equal(auditoria.detalles.version_nueva, 2);
});

test('actualizarPack rechaza otra vertical y una version inexistente', async () => {
  const sufijo = randomUUID().slice(0, 8);
  const modelo = `modelo_${sufijo}`;
  const vertical = `mv_${sufijo}`;
  const packCodigo = `pack_mv_${sufijo}`;
  creados.push({ vertical, modelo, packCodigo });

  await packs.crearModelo({ codigo: modelo, nombre: 'Modelo rechazo upgrade' });
  await packs.crearVertical({ codigo: vertical, nombre: 'Vertical rechazo upgrade', modelo_operacion: modelo });
  await packs.crear({
    codigo: packCodigo,
    version: 1,
    vertical_codigo: vertical,
    nombre: 'Pack rechazo upgrade',
    capacidades: [{ codigo: 'catalogo_items' }],
  });
  const empresa = await crearTenant({ vertical, packCodigo, sufijo });

  await assert.rejects(
    empresas.actualizarPack(empresa.id, 'pack_flota', 1),
    (error) => error?.codigo === 'PACK_VERTICAL_INCOMPATIBLE',
  );
  await assert.rejects(
    empresas.actualizarPack(empresa.id, packCodigo, 99),
    (error) => error?.codigo === 'PACK_INVALIDO',
  );
  assert.equal(
    (await client.query('SELECT pack_version FROM identity.empresas WHERE id = $1', [empresa.id])).rows[0].pack_version,
    1,
  );
});
