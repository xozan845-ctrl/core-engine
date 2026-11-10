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

// Cadena completa del catálogo: crea las fuentes legacy, las migra a items y
// las retira. Así la suite prueba el esquema final, no un estado intermedio.
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

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    // La migración 48 retira las tablas legacy: si el esquema final ya está
    // aplicado no se reejecuta la cadena destructiva.
    const { rows: [{ final }] } = await client.query(
      `SELECT to_regclass('catalog.items') IS NOT NULL
              AND to_regclass('catalog.productos') IS NULL AS final`,
    );
    if (final) return;
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

test('items es la única fuente y admite variantes e imágenes', { concurrency: false }, async () => {
  const empresaId = randomUUID();
  const padreId = randomUUID();
  const varianteId = randomUUID();
  const imagenId = randomUUID();
  const objetoId = randomUUID();
  await client.query('SELECT pg_advisory_lock(38194201)');
  await client.query('BEGIN');
  try {
    await client.query(
      `INSERT INTO catalog.items
         (id, empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock,
          marca, codigo_barras, unidad)
       VALUES ($1, $2, 'producto', 'PADRE-01', 'Producto padre', 1500, true, 7,
               'Core', '750000000001', 'Unidad (u)')`,
      [padreId, empresaId],
    );
    await client.query(
      `INSERT INTO catalog.items
         (id, empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock,
          parent_item_id)
       VALUES ($1, $2, 'producto', 'PADRE-01-ROJO', 'Variante roja', 1600, true, 3, $3)`,
      [varianteId, empresaId, padreId],
    );
    await client.query(
      `INSERT INTO catalog.item_imagenes (id, empresa_id, item_id, objeto_id, orden, es_principal)
       VALUES ($1, $2, $3, $4, 0, true)`,
      [imagenId, empresaId, padreId, objetoId],
    );
    const { rows: [item] } = await client.query(
      `SELECT i.codigo, i.marca, i.codigo_barras, i.unidad, i.parent_item_id,
              (SELECT COUNT(*)::int FROM catalog.item_imagenes ii WHERE ii.item_id = i.id) AS imagenes
         FROM catalog.items i WHERE i.id = $1`,
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
      'SELECT parent_item_id FROM catalog.items WHERE id = $1',
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
    const { rows: [legacy] } = await client.query(
      `SELECT to_regclass('catalog.productos') AS productos,
              to_regclass('catalog.servicios') AS servicios,
              to_regclass('catalog.producto_imagenes') AS imagenes`,
    );
    assert.deepEqual(legacy, { productos: null, servicios: null, imagenes: null });
    await client.query('ROLLBACK');
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
