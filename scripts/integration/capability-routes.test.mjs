import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';
import request from 'supertest';

/**
 * Enforcement por ruta (ADR-0001/R-AR-5/R-I-12).
 *
 * Levanta tres controladores reales con el CapabilityGuard global y una
 * identidad interna mínima que consulta PostgreSQL. Comprueba que la misma
 * ruta se permite o rechaza según el pack del tenant.
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

process.env.DATABASE_URL = connectionString;

const MIGRACIONES_LEGACY = [
  '01_esquemas.sql',
  '01c_cqrs_catalog.sql',
  '01e_adr25_empresas.sql',
  '01f_slugs.sql',
  '01h_catalogo_por_empresa.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
  '63_gating_capacidades.sql',
];

const MIGRACIONES_FINAL = [
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
  '63_gating_capacidades.sql',
];

const TENANTS = [
  { vertical: 'retail', pack: 'pack_retail' },
  { vertical: 'farmacia', pack: 'pack_farmacia' },
  { vertical: 'belleza', pack: 'pack_belleza' },
  { vertical: 'flota', pack: 'pack_flota' },
];

let client;
let identityServer;
let app;
let http;
const tenants = new Map();

async function crearTenant({ vertical, pack }) {
  const usuarioId = randomUUID();
  const empresaId = randomUUID();
  const sufijo = randomUUID().slice(0, 8);
  await client.query(
    `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
     VALUES ($1, 'Empresario ruta', $2, 'hash', 'empresario')`,
    [usuarioId, `rutas-${usuarioId}@test.dev`],
  );
  await client.query(
    `INSERT INTO identity.empresas
       (id, nombre, slug, modalidad_logistica, vertical_id, pack_codigo, pack_version, creado_por)
     VALUES ($1, $2, $3, NULL, $4, $5, 1, $6)`,
    [empresaId, `Ruta ${vertical} ${sufijo}`, `ruta-${vertical}-${sufijo}`, vertical, pack, usuarioId],
  );
  await client.query(
    `INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
     SELECT $1, capacidad_codigo, config, activo
       FROM identity.pack_capacidades
      WHERE pack_codigo = $2 AND pack_version = 1`,
    [empresaId, pack],
  );
  return { empresaId, usuarioId };
}

function headers(vertical) {
  const tenant = tenants.get(vertical);
  return {
    'x-user-id': tenant.usuarioId,
    'x-user-rol': 'empresario',
    'x-tenant': tenant.empresaId,
  };
}

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
    for (const name of migraciones) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
    for (const definicion of TENANTS) {
      tenants.set(definicion.vertical, await crearTenant(definicion));
    }
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  identityServer = createServer(async (req, res) => {
    if (req.method !== 'POST' || req.url !== '/internal/empresas/capacidades') {
      res.statusCode = 404;
      res.end();
      return;
    }
    let raw = '';
    for await (const chunk of req) raw += chunk;
    const { empresa_ids: empresaIds, capacidades } = JSON.parse(raw);
    const { rows } = await client.query(
      `SELECT empresa_id, capacidad_codigo, activo
         FROM identity.empresa_capacidades
        WHERE empresa_id = ANY($1::uuid[])
          AND capacidad_codigo = ANY($2::text[])`,
      [empresaIds, capacidades],
    );
    res.setHeader('content-type', 'application/json');
    res.end(JSON.stringify({ capacidades: rows }));
  });
  await new Promise((resolve) => identityServer.listen(0, '127.0.0.1', resolve));
  process.env.IDENTITY_SERVICE_URL = `http://127.0.0.1:${identityServer.address().port}`;

  const [{ Module }, { NestFactory }, shared] = await Promise.all([
    import('@nestjs/common'),
    import('@nestjs/core'),
    import('@core/shared'),
  ]);
  const [{ InventarioController }, { InventarioService }] = await Promise.all([
    import('../../packages/catalog-service/dist/inventario/inventario.controller.js'),
    import('../../packages/catalog-service/dist/inventario/inventario.service.js'),
  ]);
  const [{ FieldController }, { FieldService }] = await Promise.all([
    import('../../packages/field-service/dist/field/field.controller.js'),
    import('../../packages/field-service/dist/field/field.service.js'),
  ]);

  const inventarioStub = {
    listar: async () => [],
    listarLotes: async () => [],
  };
  const fieldStub = { listarRutas: async () => [] };

  class RutasModule {}
  Module({
    controllers: [InventarioController, FieldController],
    providers: [
      { provide: InventarioService, useValue: inventarioStub },
      { provide: FieldService, useValue: fieldStub },
    ],
  })(RutasModule);

  app = await NestFactory.create(RutasModule, { logger: false });
  app.useGlobalGuards(new shared.CapabilityGuard());
  app.useGlobalFilters(new shared.DomainErrorFilter());
  await app.init();
  http = request(app.getHttpServer());
});

after(async () => {
  await app?.close();
  await new Promise((resolve) => identityServer?.close(resolve));
  for (const tenant of tenants.values()) {
    await client.query('DELETE FROM identity.empresa_capacidades WHERE empresa_id = $1', [tenant.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.empresas WHERE id = $1', [tenant.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.usuarios WHERE id = $1', [tenant.usuarioId]).catch(() => undefined);
  }
  await client?.end();
});

test('retail permite inventario y belleza recibe CAPACIDAD_NO_ACTIVA', async () => {
  await http.get('/api/v2/admin/inventario').set(headers('retail')).expect(200);
  const respuesta = await http.get('/api/v2/admin/inventario').set(headers('belleza')).expect(403);
  assert.equal(respuesta.body.codigo, 'CAPACIDAD_NO_ACTIVA');
  assert.ok(respuesta.body.detalles.capacidades.includes('inventario_stock'));
});

test('flota permite rutas y retail no', async () => {
  await http.get('/api/v2/field/rutas').set(headers('flota')).expect(200);
  const respuesta = await http.get('/api/v2/field/rutas').set(headers('retail')).expect(403);
  assert.equal(respuesta.body.codigo, 'CAPACIDAD_NO_ACTIVA');
  assert.deepEqual(
    [...respuesta.body.detalles.capacidades].sort(),
    ['logistica_rutas', 'operacion_campo'],
  );
});

test('farmacia permite lotes y retail no', async () => {
  await http
    .get(`/api/v2/admin/inventario/lotes?empresa_id=${tenants.get('farmacia').empresaId}`)
    .set(headers('farmacia'))
    .expect(200);
  const respuesta = await http
    .get(`/api/v2/admin/inventario/lotes?empresa_id=${tenants.get('retail').empresaId}`)
    .set(headers('retail'))
    .expect(403);
  assert.equal(respuesta.body.codigo, 'CAPACIDAD_NO_ACTIVA');
  assert.ok(respuesta.body.detalles.capacidades.includes('inventario_lotes'));
});

test('sin tenant la ruta protegida falla cerrada', async () => {
  const respuesta = await http
    .get('/api/v2/admin/inventario')
    .set('x-user-id', 'u-sin-tenant')
    .set('x-user-rol', 'empresario')
    .expect(400);
  assert.equal(respuesta.body.codigo, 'TENANT_REQUERIDO');
});
