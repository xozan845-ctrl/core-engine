import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import test, { after, before } from 'node:test';
import pg from 'pg';
import { asegurarEsquemaFinal } from './esquema.mjs';

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

// Esquema final del corte: item/pricing/inventory y `catalog` retirado.
let client;

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    await asegurarEsquemaFinal(client);
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});

after(async () => {
  await client?.end();
});

test('item.items es la única fuente y admite variantes e imágenes', { concurrency: false }, async () => {
  const empresaId = randomUUID();
  const padreId = randomUUID();
  const varianteId = randomUUID();
  const imagenId = randomUUID();
  const objetoId = randomUUID();
  await client.query('SELECT pg_advisory_lock(38194201)');
  await client.query('BEGIN');
  try {
    await client.query(
      `INSERT INTO item.items
         (id, empresa_id, tipo, codigo, nombre, track_inventory,
          marca, codigo_barras, unidad)
       VALUES ($1, $2, 'producto', 'PADRE-01', 'Producto padre', true,
               'Core', '750000000001', 'Unidad (u)')`,
      [padreId, empresaId],
    );
    await client.query(
      `INSERT INTO item.items
         (id, empresa_id, tipo, codigo, nombre, track_inventory, parent_item_id)
       VALUES ($1, $2, 'producto', 'PADRE-01-ROJO', 'Variante roja', true, $3)`,
      [varianteId, empresaId, padreId],
    );
    await client.query(
      `INSERT INTO item.item_imagenes (id, empresa_id, item_id, objeto_id, orden, es_principal)
       VALUES ($1, $2, $3, $4, 0, true)`,
      [imagenId, empresaId, padreId, objetoId],
    );
    const { rows: [item] } = await client.query(
      `SELECT i.codigo, i.marca, i.codigo_barras, i.unidad, i.parent_item_id,
              (SELECT COUNT(*)::int FROM item.item_imagenes ii WHERE ii.item_id = i.id) AS imagenes
         FROM item.items i WHERE i.id = $1`,
      [padreId],
    );
    assert.deepEqual(item, {
      codigo: 'PADRE-01',
      marca: 'Core',
      codigo_barras: '750000000001',
      unidad: 'Unidad (u)',
      parent_item_id: null,
      imagenes: 1,
    });
    const { rows: [variante] } = await client.query(
      'SELECT parent_item_id FROM item.items WHERE id = $1',
      [varianteId],
    );
    assert.equal(variante.parent_item_id, padreId);
    await client.query('ROLLBACK');
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});

test('las ofertas usan item_id y ya no conservan columnas legacy', { concurrency: false }, async () => {
  const empresaId = randomUUID();
  const tiendaId = randomUUID();
  const itemId = randomUUID();
  await client.query('SELECT pg_advisory_lock(38194201)');
  await client.query('BEGIN');
  try {
    await client.query(
      `INSERT INTO item.items
        (id, empresa_id, tipo, codigo, nombre, track_inventory)
       VALUES ($1, $2, 'alquiler', 'ALQ-001', 'Alquiler item', false)`,
      [itemId, empresaId],
    );
    await client.query(
      `INSERT INTO stores.tiendas (id, vendedor_id, empresa_id, slug, nombre)
       VALUES ($1, $2, $3, $4, 'Tienda item')`,
      [tiendaId, randomUUID(), empresaId, `tienda-${randomUUID().slice(0, 8)}`],
    );
    await client.query(
      `INSERT INTO stores.ofertas
        (tienda_id, item_type, item_id, sku, producto_nombre,
         margen, precio_base_cents, precio_venta_cents, stock)
       VALUES ($1, 'alquiler', $2, 'ALQ-001', 'Alquiler item', 10, 5000, 5500, NULL)`,
      [tiendaId, itemId],
    );
    const { rows: [oferta] } = await client.query(
      `SELECT item_type, item_id, stock
         FROM stores.ofertas
        WHERE tienda_id = $1`,
      [tiendaId],
    );
    assert.deepEqual(oferta, { item_type: 'alquiler', item_id: itemId, stock: null });
    const { rows: [retirado] } = await client.query(
      `SELECT to_regnamespace('catalog') AS catalog,
              to_regclass('item.items') AS items`,
    );
    assert.deepEqual(retirado, { catalog: null, items: 'item.items' });
    await client.query('ROLLBACK');
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
