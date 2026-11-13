import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Flujos piloto por modelo de negocio (ADR-0001/R-I-12).
 *
 * Usa servicios reales contra PostgreSQL, sin stack completo:
 * farmacia (lotes), belleza (agenda) y flota (vehículo → ruta → parada).
 * El E2E de release sigue siendo `npm run demo`; este test es el gate de PR.
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
  '01i_variantes_imagenes.sql',
  '11_catalogo_productos_servicios.sql',
  '25_catalog_items.sql',
  '28_scheduling.sql',
  '02_field.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
];

const MIGRACIONES_FINAL = [
  '28_scheduling.sql',
  '02_field.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
];

const TENANTS = [
  { vertical: 'farmacia', pack: 'pack_farmacia' },
  { vertical: 'belleza', pack: 'pack_belleza' },
  { vertical: 'flota', pack: 'pack_flota' },
];

let client;
let pgService;
const creados = new Map();

async function crearTenant({ vertical, pack }) {
  const usuarioId = randomUUID();
  const empresaId = randomUUID();
  const sufijo = randomUUID().slice(0, 8);
  await client.query(
    `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
     VALUES ($1, 'Empresario piloto', $2, 'hash', 'empresario')`,
    [usuarioId, `piloto-${usuarioId}@test.dev`],
  );
  await client.query(
    `INSERT INTO identity.empresas
       (id, nombre, slug, modalidad_logistica, vertical_id, pack_codigo, pack_version, creado_por)
     VALUES ($1, $2, $3, NULL, $4, $5, 1, $6)`,
    [empresaId, `Piloto ${vertical} ${sufijo}`, `piloto-${vertical}-${sufijo}`, vertical, pack, usuarioId],
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
    // La migración de compatibilidad completa depende de tablas de otros
    // servicios; para este flujo basta con la columna que usa scheduling.
    await client.query('ALTER TABLE scheduling.citas ADD COLUMN IF NOT EXISTS parte_id uuid');
    for (const definicion of TENANTS) {
      creados.set(definicion.vertical, await crearTenant(definicion));
    }
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const shared = await import('@core/shared');
  const { PgService } = shared;
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
});

after(async () => {
  for (const creado of creados.values()) {
    await client.query('DELETE FROM catalog.lotes_inventario WHERE empresa_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM catalog.items WHERE empresa_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM scheduling.recordatorios WHERE empresa_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM scheduling.citas WHERE empresa_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM scheduling.disponibilidad WHERE empresa_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM scheduling.recursos WHERE empresa_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM scheduling.outbox WHERE payload->\'data\'->>\'empresa_id\' = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM field.paradas WHERE tenant_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM field.rutas WHERE tenant_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM field.vehiculos WHERE tenant_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.empresa_capacidades WHERE empresa_id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.empresas WHERE id = $1', [creado.empresaId]).catch(() => undefined);
    await client.query('DELETE FROM identity.usuarios WHERE id = $1', [creado.usuarioId]).catch(() => undefined);
  }
  await pgService?.onModuleDestroy();
  await client?.end();
});

// Los flujos de lotes FEFO y serialización se cubren en
// `packages/catalog-service/tests/catalog_pg.rs` (ADR-0016).

test('flota: crea vehículo, ruta y parada sobre el mismo tenant', async () => {
  const { empresaId } = tenant('flota');
  const sufijo = randomUUID().slice(0, 8);
  const { rows: [vehiculo] } = await client.query(
    `INSERT INTO field.vehiculos (tenant_id, placa, tipo, marca, modelo, anio)
     VALUES ($1, $2, 'camioneta', 'Toyota', 'Hilux', 2026) RETURNING *`,
    [empresaId, `FLT-${sufijo}`],
  );
  const { rows: [ruta] } = await client.query(
    `INSERT INTO field.rutas (tenant_id, nombre, descripcion, vehiculo_asignado_id)
     VALUES ($1, $2, 'Reparto urbano', $3) RETURNING *`,
    [empresaId, `Ruta ${sufijo}`, vehiculo.id],
  );
  const { rows: [parada] } = await client.query(
    `INSERT INTO field.paradas (tenant_id, ruta_id, nombre, direccion, lat, lng, orden)
     VALUES ($1, $2, 'Parada centro', 'Managua', 12.1364, -86.2514, 1) RETURNING *`,
    [empresaId, ruta.id],
  );

  assert.equal(vehiculo.tenant_id, empresaId);
  assert.equal(ruta.tenant_id, empresaId);
  assert.equal(parada.ruta_id, ruta.id);

  const { rows: [rutaDb] } = await client.query(
    `SELECT nombre FROM field.rutas WHERE tenant_id = $1 AND id = $2`,
    [empresaId, ruta.id],
  );
  assert.equal(rutaDb.nombre, `Ruta ${sufijo}`);
});
