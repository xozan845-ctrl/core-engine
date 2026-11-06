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
  '01f_slugs.sql',
  '01h_catalogo_por_empresa.sql',
  '01i_variantes_imagenes.sql',
  '11_catalogo_productos_servicios.sql',
  '25_catalog_items.sql',
  '26_stores_items.sql',
];

let client;

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    const { rows: [{ existe }] } = await client.query(
      'SELECT to_regnamespace(\'identity\') IS NOT NULL AS existe',
    );
    const migraciones = existe ? MIGRACIONES.filter((name) => name !== '01_esquemas.sql') : MIGRACIONES;
    for (const name of migraciones) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});

after(async () => {
  await client?.end();
});

test('los productos y servicios legacy se proyectan como items estables', { concurrency: false }, async () => {
  const empresaId = randomUUID();
  const sku = `ITEM-${randomUUID().slice(0, 8).toUpperCase()}`;
  const codigoServicio = `SERV-${randomUUID().slice(0, 8).toUpperCase()}`;
  await client.query('SELECT pg_advisory_lock(38194201)');
  await client.query('BEGIN');
  try {
    await client.query(
      `INSERT INTO catalog.productos (empresa_id, sku, nombre, precio_base_cents, stock)
       VALUES ($1, $2, 'Producto item', 1500, 7)`,
      [empresaId, sku],
    );
    await client.query(
      `INSERT INTO catalog.servicios (empresa_id, codigo, nombre, precio_cents, duracion_minutos, atributos)
       VALUES ($1, $2, 'Servicio item', 3000, 45, $3::jsonb)`,
      [empresaId, codigoServicio, JSON.stringify({ modalidad: 'presencial' })],
    );
    const { rows } = await client.query(
      `SELECT tipo, codigo, track_inventory, stock, duration_minutes, atributos
         FROM catalog.items_vista
        WHERE empresa_id = $1
        ORDER BY tipo`,
      [empresaId],
    );
    assert.deepEqual(rows, [
      {
        tipo: 'producto',
        codigo: sku,
        track_inventory: true,
        stock: 7,
        duration_minutes: null,
        atributos: {},
      },
      {
        tipo: 'servicio',
        codigo: codigoServicio,
        track_inventory: false,
        stock: null,
        duration_minutes: 45,
        atributos: { modalidad: 'presencial' },
      },
    ]);
    await client.query('ROLLBACK');
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});

test('los alquileres y ofertas aceptan la superficie unificada', { concurrency: false }, async () => {
  const empresaId = randomUUID();
  const tiendaId = randomUUID();
  const itemId = randomUUID();
  await client.query('SELECT pg_advisory_lock(38194201)');
  await client.query('BEGIN');
  try {
    await client.query(
      `INSERT INTO catalog.items
        (id, empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock)
       VALUES ($1, $2, 'alquiler', 'ALQ-001', 'Alquiler item', 5000, false, NULL)`,
      [itemId, empresaId],
    );
    await client.query(
      `INSERT INTO stores.tiendas (id, vendedor_id, empresa_id, slug, nombre)
       VALUES ($1, $2, $3, $4, 'Tienda item')`,
      [tiendaId, randomUUID(), empresaId, `tienda-${randomUUID().slice(0, 8)}`],
    );
    await client.query(
      `INSERT INTO stores.ofertas
        (tienda_id, producto_id, item_type, item_id, sku, producto_nombre,
         margen, precio_base_cents, precio_venta_cents, stock)
       VALUES ($1, NULL, 'alquiler', $2, 'ALQ-001', 'Alquiler item', 10, 5000, 5500, NULL)`,
      [tiendaId, itemId],
    );
    const { rows: [oferta] } = await client.query(
      `SELECT item_type, item_id, producto_id, stock
         FROM stores.ofertas
        WHERE tienda_id = $1`,
      [tiendaId],
    );
    assert.deepEqual(oferta, { item_type: 'alquiler', item_id: itemId, producto_id: null, stock: null });
    await client.query('ROLLBACK');
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
