import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Multi-bodega y stock por ubicacion (Fase P1.2).
 * Verifica la asignacion inicial, transferencias, el agregado `items.stock` y
 * el trigger que distribuye los cambios directos a la ubicacion predeterminada.
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
];

let client;
let pgService;
let ubicaciones;
const empresaId = randomUUID();
const itemId = randomUUID();
let central;
let sucursal;

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
    await aplicar('65_inventario_ubicaciones.sql');
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const [{ PgService }, { UbicacionesService }] = await Promise.all([
    import('@core/shared'),
    import('../../packages/catalog-service/dist/inventario/ubicaciones.service.js'),
  ]);
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
  ubicaciones = new UbicacionesService(pgService);
});

after(async () => {
  await client.query('DELETE FROM catalog.ubicaciones WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await client.query('DELETE FROM catalog.items WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await pgService?.onModuleDestroy();
  await client?.end();
});

test('gestiona stock por ubicacion y mantiene el total del item', { concurrency: false }, async () => {
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    central = await ubicaciones.crear(empresaId, {
      codigo: 'central',
      nombre: 'Bodega central',
      predeterminada: true,
    });
    sucursal = await ubicaciones.crear(empresaId, {
      codigo: 'sucursal',
      nombre: 'Sucursal',
    });
    assert.notEqual(central.id, sucursal.id);

    await client.query(
      `INSERT INTO catalog.items
         (id, empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock)
       VALUES ($1, $2, 'producto', 'UBI-01', 'Item ubicaciones', 1000, true, 10)`,
      [itemId, empresaId],
    );

    const desgloseInicial = await ubicaciones.stockDeItem(empresaId, itemId);
    assert.equal(desgloseInicial.stock_total, 10);
    assert.equal(desgloseInicial.total_ubicaciones, 10);
    assert.deepEqual(
      desgloseInicial.ubicaciones.map((fila) => [fila.codigo, fila.stock]),
      [['central', 10]],
    );

    const ajuste = await ubicaciones.ajustarStock(empresaId, sucursal.id, itemId, 4);
    assert.equal(ajuste.stock, 4);
    assert.equal(ajuste.stock_total, 14);

    const traslado = await ubicaciones.trasladar(empresaId, {
      item_id: itemId,
      origen_id: central.id,
      destino_id: sucursal.id,
      cantidad: 3,
    });
    assert.equal(traslado.origen.stock, 7);
    assert.equal(traslado.destino.stock, 7);

    // Los cambios directos de stock (ordenes/ajustes) bajan primero de la
    // ubicacion predeterminada y nunca pierden el agregado.
    await client.query('UPDATE catalog.items SET stock = stock - 5 WHERE id = $1', [itemId]);
    const trasVenta = await ubicaciones.stockDeItem(empresaId, itemId);
    assert.equal(trasVenta.stock_total, 9);
    assert.equal(trasVenta.total_ubicaciones, 9);
    assert.deepEqual(
      Object.fromEntries(trasVenta.ubicaciones.map((fila) => [fila.codigo, fila.stock])),
      { central: 2, sucursal: 7 },
    );

    await client.query('UPDATE catalog.items SET stock = stock + 2 WHERE id = $1', [itemId]);
    const trasIngreso = await ubicaciones.stockDeItem(empresaId, itemId);
    assert.equal(trasIngreso.stock_total, 11);
    assert.equal(trasIngreso.total_ubicaciones, 11);
    assert.deepEqual(
      Object.fromEntries(trasIngreso.ubicaciones.map((fila) => [fila.codigo, fila.stock])),
      { central: 4, sucursal: 7 },
    );

    await assert.rejects(
      ubicaciones.trasladar(empresaId, {
        item_id: itemId,
        origen_id: central.id,
        destino_id: sucursal.id,
        cantidad: 99,
      }),
      (error) => error?.codigo === 'STOCK_UBICACION_INSUFICIENTE',
    );

    await assert.rejects(
      ubicaciones.actualizar(empresaId, central.id, { activo: false }),
      (error) => error?.codigo === 'CONFLICTO',
    );

    const vacia = await ubicaciones.crear(empresaId, {
      codigo: 'temporal',
      nombre: 'Temporal',
    });
    const desactivada = await ubicaciones.actualizar(empresaId, vacia.id, { activo: false });
    assert.equal(desactivada.activo, false);

    const stockSucursal = await ubicaciones.listarStock(empresaId, sucursal.id);
    assert.deepEqual(stockSucursal.map((fila) => fila.stock), [7]);
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
