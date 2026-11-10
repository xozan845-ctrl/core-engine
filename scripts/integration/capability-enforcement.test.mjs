import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Enforcement real de capacidades (ADR-0001/R-AR-5/R-I-12).
 *
 * El guard consulta PostgreSQL de verdad: no hay `vi.fn` ni una lista de
 * capacidades inyectada. Cada tenant se crea con su pack y se comprueba que
 * la misma operación se permite o se rechaza según la vertical.
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
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
];

const TENANTS = [
  { vertical: 'retail', pack: 'pack_retail', tipo: 'comercial' },
  { vertical: 'farmacia', pack: 'pack_farmacia', tipo: 'comercial' },
  { vertical: 'belleza', pack: 'pack_belleza', tipo: 'comercial' },
  { vertical: 'dental', pack: 'pack_dental', tipo: 'comercial' },
  { vertical: 'flota', pack: 'pack_flota', tipo: 'logistica' },
];

let client;
let pgService;
let guard;
let CapabilityGuard;
let CAPACIDADES;
const creados = new Map();

function reflectorCon(requeridas) {
  return { getAllAndMerge: () => requeridas ?? [] };
}

function contextoCon(tenantId, usuarioId = 'u-guard') {
  return {
    switchToHttp: () => ({
      getRequest: () => ({
        headers: {
          'x-user-id': usuarioId,
          'x-user-rol': 'empresario',
          ...(tenantId ? { 'x-tenant': tenantId } : {}),
        },
      }),
    }),
    getHandler: () => ({}),
    getClass: () => ({}),
  };
}

async function crearTenant({ vertical, pack, tipo }) {
  const usuarioId = randomUUID();
  const empresaId = randomUUID();
  const sufijo = randomUUID().slice(0, 8);
  await client.query(
    `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
     VALUES ($1, 'Empresario guard', $2, 'hash', 'empresario')`,
    [usuarioId, `guard-${usuarioId}@test.dev`],
  );
  await client.query(
    `INSERT INTO identity.empresas
       (id, nombre, slug, modalidad_logistica, vertical_id, pack_codigo, pack_version,
        creado_por, tipo_empresa)
     VALUES ($1, $2, $3, NULL, $4, $5, 1, $6, $7)`,
    [empresaId, `Tenant ${vertical} ${sufijo}`, `tenant-${vertical}-${sufijo}`,
      vertical, pack, usuarioId, tipo],
  );
  await client.query(
    `INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
     SELECT $1, capacidad_codigo, config, activo
       FROM identity.pack_capacidades
      WHERE pack_codigo = $2 AND pack_version = 1`,
    [empresaId, pack],
  );
  return { empresaId, usuarioId, vertical };
}

async function limpiarTenant({ empresaId, usuarioId }) {
  await client.query('DELETE FROM identity.empresa_capacidades WHERE empresa_id = $1', [empresaId]);
  await client.query('DELETE FROM identity.empresa_miembros WHERE empresa_id = $1', [empresaId]);
  await client.query('DELETE FROM identity.empresas WHERE id = $1', [empresaId]);
  await client.query('DELETE FROM identity.usuarios WHERE id = $1', [usuarioId]);
}

function tenant(vertical) {
  return creados.get(vertical);
}

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    const { rows: [{ existe }] } = await client.query(
      `SELECT to_regnamespace('identity') IS NOT NULL AS existe`,
    );
    const migraciones = existe ? MIGRACIONES.filter((name) => name !== '01_esquemas.sql') : MIGRACIONES;
    for (const name of migraciones) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
    for (const definicion of TENANTS) {
      const creado = await crearTenant(definicion);
      creados.set(definicion.vertical, creado);
    }
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const shared = await import('@core/shared');
  const { PgService } = shared;
  CapabilityGuard = shared.CapabilityGuard;
  CAPACIDADES = shared.CAPACIDADES;
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
  guard = new CapabilityGuard(pgService, reflectorCon());
});

after(async () => {
  for (const creado of creados.values()) {
    await limpiarTenant(creado).catch(() => undefined);
  }
  await pgService?.onModuleDestroy();
  await client?.end();
});

async function permite(vertical, capacidades) {
  guard = new CapabilityGuard(pgService, reflectorCon(capacidades));
  return guard.canActivate(contextoCon(tenant(vertical).empresaId, tenant(vertical).usuarioId));
}

async function rechaza(vertical, capacidades) {
  guard = new CapabilityGuard(pgService, reflectorCon(capacidades));
  await assert.rejects(
    guard.canActivate(contextoCon(tenant(vertical).empresaId, tenant(vertical).usuarioId)),
    (error) => {
      assert.equal(error.codigo, 'CAPACIDAD_NO_ACTIVA');
      assert.deepEqual(error.detalles, {
        empresa_id: tenant(vertical).empresaId,
        capacidades,
      });
      return true;
    },
  );
}

test('retail permite catálogo, stock y facturación', async () => {
  await assert.doesNotReject(
    permite('retail', [
      CAPACIDADES.CATALOGO_ITEMS,
      CAPACIDADES.INVENTARIO_STOCK,
      CAPACIDADES.FACTURACION,
    ]),
  );
});

test('retail no puede usar agenda de servicios', async () => {
  await rechaza('retail', [CAPACIDADES.AGENDA_RECURSOS]);
});

test('belleza permite agenda pero no inventario', async () => {
  await assert.doesNotReject(permite('belleza', [CAPACIDADES.AGENDA_RECURSOS]));
  await rechaza('belleza', [CAPACIDADES.INVENTARIO_STOCK]);
});

test('farmacia permite lotes y retail no', async () => {
  await assert.doesNotReject(permite('farmacia', [CAPACIDADES.INVENTARIO_LOTES]));
  await rechaza('retail', [CAPACIDADES.INVENTARIO_LOTES]);
});

test('dental permite agenda y cotizaciones', async () => {
  await assert.doesNotReject(
    permite('dental', [CAPACIDADES.AGENDA_RECURSOS, CAPACIDADES.COTIZACIONES]),
  );
});

test('flota permite rutas y retail no', async () => {
  await assert.doesNotReject(permite('flota', [CAPACIDADES.LOGISTICA_RUTAS]));
  await rechaza('retail', [CAPACIDADES.LOGISTICA_RUTAS]);
});

test('una capacidad activa en retail no autoriza a otro tenant', async () => {
  await assert.doesNotReject(permite('retail', [CAPACIDADES.INVENTARIO_STOCK]));
  await rechaza('belleza', [CAPACIDADES.INVENTARIO_STOCK]);
});

test('desactivar la capacidad en la empresa la revoca de inmediato', async () => {
  const retail = tenant('retail');
  await client.query(
    `UPDATE identity.empresa_capacidades
        SET activo = false
      WHERE empresa_id = $1 AND capacidad_codigo = $2`,
    [retail.empresaId, CAPACIDADES.INVENTARIO_STOCK],
  );
  try {
    await rechaza('retail', [CAPACIDADES.INVENTARIO_STOCK]);
  } finally {
    await client.query(
      `UPDATE identity.empresa_capacidades
          SET activo = true
        WHERE empresa_id = $1 AND capacidad_codigo = $2`,
      [retail.empresaId, CAPACIDADES.INVENTARIO_STOCK],
    );
  }
});

test('sin tenant activo el guard falla cerrado', async () => {
  guard = new CapabilityGuard(pgService, reflectorCon([CAPACIDADES.CATALOGO_ITEMS]));
  await assert.rejects(
    guard.canActivate(contextoCon(null)),
    (error) => error.codigo === 'TENANT_REQUERIDO',
  );
});
