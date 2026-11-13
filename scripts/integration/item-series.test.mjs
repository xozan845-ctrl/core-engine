import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Serial/IMEI por unidad (Fase P1.3).
 * Verifica el registro de unidades, la venta con garantia, la devolucion y
 * el fail-closed cuando un item serializado no tiene unidades disponibles.
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
  '53_reservas_lotes.sql',
  '60_catalog_item_componentes.sql',
  '65_inventario_ubicaciones.sql',
  '66_item_series.sql',
];

let client;
let pgService;
let series;
let productos;
let itemsService;
const empresaId = randomUUID();
const itemId = randomUUID();
const itemSinSeriesId = randomUUID();
let ubicacion;

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
    // Dependencias directas: en la BD compartida el esquema ya es final y
    // estas migraciones pueden no haberse aplicado aun.
    await aplicar('53_reservas_lotes.sql');
    await aplicar('60_catalog_item_componentes.sql');
    await aplicar('65_inventario_ubicaciones.sql');
    await aplicar('66_item_series.sql');
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const [{ PgService }, { SeriesService }, { ProductosService }, { UbicacionesService }, { ItemsService }] =
    await Promise.all([
      import('@core/shared'),
      import('../../packages/catalog-service/dist/items/series.service.js'),
      import('../../packages/catalog-service/dist/productos/productos.service.js'),
      import('../../packages/catalog-service/dist/inventario/ubicaciones.service.js'),
      import('../../packages/catalog-service/dist/items/items.service.js'),
    ]);
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
  series = new SeriesService(pgService);
  productos = new ProductosService(pgService);
  itemsService = new ItemsService(pgService);
  const ubicaciones = new UbicacionesService(pgService);

  ubicacion = await ubicaciones.crear(empresaId, {
    codigo: 'central',
    nombre: 'Bodega central',
    predeterminada: true,
  });
  await client.query(
    `INSERT INTO catalog.items
       (id, empresa_id, tipo, codigo, nombre, precio_cents, costo_cents,
        track_inventory, stock, garantia_meses)
     VALUES ($1, $2, 'producto', 'SER-01', 'Telefono serializado', 100000, 60000, true, 2, 12),
            ($3, $2, 'producto', 'SIN-SERIE', 'Item sin unidades', 5000, 2000, true, 1, 6)`,
    [itemId, empresaId, itemSinSeriesId],
  );
});

after(async () => {
  await client.query('DELETE FROM catalog.ajustes_stock WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await client.query('DELETE FROM catalog.ubicaciones WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await client.query('DELETE FROM catalog.items WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await pgService?.onModuleDestroy();
  await client?.end();
});

test('vende, garantiza y devuelve unidades serializadas', { concurrency: false }, async () => {
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    await series.habilitarSerializacion(empresaId, itemId, true);
    await series.crear(empresaId, itemId, { serial: 'SN-001', ubicacion_id: ubicacion.id });
    await series.crear(empresaId, itemId, {
      serial: 'SN-002',
      imei: '356789012345678',
      ubicacion_id: ubicacion.id,
    });
    await assert.rejects(
      series.crear(empresaId, itemId, { serial: 'sn-001' }),
      (error) => error?.codigo === 'CONFLICTO',
    );

    const detalle = await series.buscarPorSerial(empresaId, 'SN-001');
    assert.equal(detalle.estado, 'disponible');
    assert.equal(detalle.garantia_meses, 12);
    assert.equal(detalle.garantia_vigente, false);

    const orderId = randomUUID();
    const reserva = await pgService.transaccion((tx) =>
      productos.reservarStockEnTransaccion(tx, orderId, [
        { empresa_id: empresaId, sku: 'SER-01', cantidad: 2, item_id: itemId },
      ]),
    );
    assert.equal(reserva.ok, true);

    const vendidas = await series.listar(empresaId, itemId);
    assert.deepEqual(vendidas.map((fila) => fila.estado), ['vendido', 'vendido']);
    assert.ok(vendidas.every((fila) => fila.order_id === orderId));
    assert.ok(vendidas.every((fila) => fila.vendido_en !== null));
    assert.ok(vendidas.every((fila) => fila.garantia_hasta !== null));
    assert.ok(vendidas.every((fila) => fila.ubicacion_id === null));

    const vendida = await series.buscarPorSerial(empresaId, 'SN-002');
    assert.equal(vendida.estado, 'vendido');
    assert.equal(vendida.order_id, orderId);
    assert.equal(vendida.garantia_vigente, true);

    const stockVendido = await client.query(
      'SELECT stock FROM catalog.items WHERE id = $1',
      [itemId],
    );
    assert.equal(stockVendido.rows[0].stock, 0);

    await pgService.transaccion(async (tx) => {
      await productos.reintegrarStockEnTransaccion(tx, orderId, [
        { empresa_id: empresaId, sku: 'SER-01', cantidad: 2 },
      ]);
      await productos.devolverSeriesEnTransaccion(tx, orderId, [
        { empresa_id: empresaId, sku: 'SER-01', cantidad: 2 },
      ]);
    });

    const devueltas = await series.listar(empresaId, itemId);
    assert.deepEqual(devueltas.map((fila) => fila.estado), ['devuelto', 'devuelto']);
    assert.ok(devueltas.every((fila) => fila.devuelto_en !== null));
    assert.ok(devueltas.every((fila) => fila.ubicacion_id === ubicacion.id));

    const stockDevuelto = await client.query(
      'SELECT stock FROM catalog.items WHERE id = $1',
      [itemId],
    );
    assert.equal(stockDevuelto.rows[0].stock, 2);

    // Un item serializado sin unidades no se vende aunque tenga stock.
    await series.habilitarSerializacion(empresaId, itemSinSeriesId, true);
    const sinUnidades = await pgService.transaccion((tx) =>
      productos.reservarStockEnTransaccion(tx, randomUUID(), [
        { empresa_id: empresaId, sku: 'SIN-SERIE', cantidad: 1, item_id: itemSinSeriesId },
      ]),
    );
    assert.equal(sinUnidades.ok, false);
    assert.equal(sinUnidades.fallidos[0]?.motivo, 'serial_insuficiente');

    const lote = await itemsService.lotePorSkus(['SIN-SERIE']);
    assert.deepEqual(lote, [{
      empresa_id: empresaId,
      sku: 'sin-serie',
      stock: 1,
      track_serial: true,
      seriales_disponibles: 0,
      estado: 'agotado',
    }]);

    const stockIntacto = await client.query(
      'SELECT stock FROM catalog.items WHERE id = $1',
      [itemSinSeriesId],
    );
    assert.equal(stockIntacto.rows[0].stock, 1);
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
