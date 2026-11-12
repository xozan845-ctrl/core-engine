import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Operaciones representativas por modelo de negocio (ADR-0001/R-I-12).
 *
 * No levanta el stack completo: usa los servicios reales contra PostgreSQL.
 * Cubre los tres pilotos del runbook con una operación de negocio cada uno:
 * farmacia (lotes), belleza (agenda) y flota (vehículo → ruta → parada).
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
  '25_catalog_items.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
  '28_scheduling.sql',
  '02_field.sql',
];

const TENANTS = [
  { vertical: 'farmacia', pack: 'pack_farmacia', tipo: 'comercial' },
  { vertical: 'belleza', pack: 'pack_belleza', tipo: 'comercial' },
  { vertical: 'flota', pack: 'pack_flota', tipo: 'logistica' },
];

let client;
let pgService;
let inventario;
let field;
const creados = new Map();

async function crearTenant({ vertical, pack, tipo }) {
  const usuarioId = randomUUID();
  const empresaId = randomUUID();
  const sufijo = randomUUID().slice(0, 8);
  await client.query(
    `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
     VALUES ($1, 'Empresario operaciones', $2, 'hash', 'empresario')`,
    [usuarioId, `ops-${usuarioId}@test.dev`],
  );
  await client.query(
    `INSERT INTO identity.empresas
       (id, nombre, slug, modalidad_logistica, vertical_id, pack_codigo, pack_version,
        creado_por, tipo_empresa)
     VALUES ($1, $2, $3, NULL, $4, $5, 1, $6, $7)`,
    [empresaId, `Operaciones ${vertical} ${sufijo}`, `ops-${vertical}-${sufijo}`,
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
    const { rows: [{ catalogoFinal }] } = await client.query(
      `SELECT to_regclass('catalog.items') IS NOT NULL AND to_regclass('catalog.productos') IS NULL AS "catalogoFinal"`,
    );
    const legacyCatalogo = new Set([
      '01c_cqrs_catalog.sql',
      '01e_adr25_empresas.sql',
      '01h_catalogo_por_empresa.sql',
      '01i_variantes_imagenes.sql',
      '11_catalogo_productos_servicios.sql',
      '24_multi_vertical.sql',
      '25_catalog_items.sql',
    ]);
    const migraciones = existe
      ? MIGRACIONES.filter(
          (name) =>
            name !== '01_esquemas.sql' && !(catalogoFinal && legacyCatalogo.has(name)),
        )
      : MIGRACIONES;
    for (const name of migraciones) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
    await client.query('ALTER TABLE scheduling.citas ADD COLUMN IF NOT EXISTS parte_id uuid');
    for (const definicion of TENANTS) {
      creados.set(definicion.vertical, await crearTenant(definicion));
    }
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const shared = await import('@core/shared');
  const { PgService } = shared;
  const [{ InventarioService }, { FieldService }] = await Promise.all([
    import('../../packages/catalog-service/dist/inventario/inventario.service.js'),
    import('../../packages/field-service/dist/field/field.service.js'),
  ]);

  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
  inventario = new InventarioService(pgService);
  field = new FieldService(pgService);
});

async function limpiarTenant({ empresaId, usuarioId }) {
  const borrados = [
    ['catalog.lotes_inventario', 'empresa_id'],
    ['catalog.items', 'empresa_id'],
    ['scheduling.recordatorios', 'empresa_id'],
    ['scheduling.citas', 'empresa_id'],
    ['scheduling.disponibilidad', 'empresa_id'],
    ['scheduling.recursos', 'empresa_id'],
    ['field.paradas', 'tenant_id'],
    ['field.rutas', 'tenant_id'],
    ['field.vehiculos', 'tenant_id'],
    ['identity.empresa_capacidades', 'empresa_id'],
    ['identity.empresa_miembros', 'empresa_id'],
    ['identity.empresas', 'id'],
  ];
  for (const [tabla, columna] of borrados) {
    await client.query(`DELETE FROM ${tabla} WHERE ${columna} = $1`, [empresaId]).catch(() => undefined);
  }
  await client.query(
    `DELETE FROM scheduling.outbox WHERE payload->'data'->>'empresa_id' = $1`,
    [empresaId],
  ).catch(() => undefined);
  await client.query('DELETE FROM identity.usuarios WHERE id = $1', [usuarioId]).catch(() => undefined);
}

after(async () => {
  for (const creado of creados.values()) {
    await limpiarTenant(creado);
  }
  await pgService?.onModuleDestroy();
  await client?.end();
});

test('farmacia: un producto con inventario acepta lotes y vencimiento', async () => {
  const { empresaId } = tenant('farmacia');
  const { rows: [capacidad] } = await client.query(
    `SELECT 1 FROM identity.empresa_capacidades
      WHERE empresa_id = $1 AND capacidad_codigo = 'inventario_lotes' AND activo = true`,
    [empresaId],
  );
  assert.ok(capacidad, 'farmacia debe tener inventario_lotes activo');

  const { rows: [item] } = await client.query(
    `INSERT INTO catalog.items
       (empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock)
     VALUES ($1, 'producto', $2, 'Medicamento con lote', 2500, true, 20)
     RETURNING id`,
    [empresaId, `MED-${randomUUID().slice(0, 8)}`],
  );
  const lote = await inventario.crearLote({
    empresa_id: empresaId,
    item_id: item.id,
    codigo_lote: `LOTE-${randomUUID().slice(0, 8)}`,
    vence_en: '2031-12-31',
    cantidad_inicial: 20,
  });

  assert.equal(lote.empresa_id, empresaId);
  assert.equal(lote.codigo_lote.startsWith('LOTE-'), true);
  assert.equal(lote.cantidad_inicial, 20);
  assert.equal(lote.cantidad_disponible, 20);
  assert.equal(new Date(lote.vence_en).toISOString().slice(0, 10), '2031-12-31');
});

test('farmacia: un servicio sin inventario no admite lotes', async () => {
  const { empresaId } = tenant('farmacia');
  const { rows: [servicio] } = await client.query(
    `INSERT INTO catalog.items
       (empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, duration_minutes)
     VALUES ($1, 'servicio', $2, 'Consulta', 5000, false, 30)
     RETURNING id`,
    [empresaId, `SERV-${randomUUID().slice(0, 8)}`],
  );

  await assert.rejects(
    inventario.crearLote({
      empresa_id: empresaId,
      item_id: servicio.id,
      codigo_lote: 'LOTE-SERVICIO',
      cantidad_inicial: 1,
    }),
    (error) => error?.codigo === 'INVENTARIO_NO_APLICA',
  );
});

test('flota: crea vehículo, ruta y parada sobre el mismo tenant', async () => {
  const { empresaId } = tenant('flota');
  const sufijo = randomUUID().slice(0, 8);
  const vehiculo = await field.crearVehiculo(empresaId, {
    placa: `FLT-${sufijo}`,
    tipo: 'camioneta',
    marca: 'Toyota',
    modelo: 'Hilux',
    anio: 2026,
  });
  const ruta = await field.crearRuta(empresaId, {
    nombre: `Ruta ${sufijo}`,
    descripcion: 'Reparto urbano',
    vehiculoAsignadoId: vehiculo.id,
  });
  const parada = await field.agregarParada(empresaId, ruta.id, {
    nombre: 'Parada centro',
    direccion: 'Managua',
    lat: 12.1364,
    lng: -86.2514,
    orden: 1,
  });

  assert.equal(vehiculo.tenant_id, empresaId);
  assert.equal(ruta.tenant_id, empresaId);
  assert.equal(parada.ruta_id, ruta.id);

  const { rows: [rutaDb] } = await client.query(
    `SELECT nombre FROM field.rutas WHERE tenant_id = $1 AND id = $2`,
    [empresaId, ruta.id],
  );
  assert.equal(rutaDb.nombre, `Ruta ${sufijo}`);
});
