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

// Cadena que deja el catálogo en su estado final (items única fuente) y el
// esquema que consume commissions-service (ADR-25/ADR-37).
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
      `SELECT to_regnamespace('identity') IS NOT NULL AS existe`,
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

test('el stock de items se aísla por empresa y las reservas no lo contaminan', { concurrency: false }, async () => {
  await client.query('SELECT pg_advisory_lock(38194201)');
  await client.query('BEGIN');
  try {
    const sku = `REG-${randomUUID()}`;
    const orderId = randomUUID();
    const empresaId = randomUUID();
    const empresaOtra = randomUUID();
    await client.query(
      `INSERT INTO catalog.items
         (empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock)
       VALUES ($1, 'producto', $2, 'Producto de regresion', 1000, true, 10),
              ($3, 'producto', $2, 'Mismo codigo en otra empresa', 2000, true, 4)`,
      [empresaId, sku, empresaOtra],
    );
    await assertStock(client, empresaId, sku, 10);
    await assertStock(client, empresaOtra, sku, 4);

    await client.query(
      `UPDATE catalog.items SET stock = stock - 1
        WHERE empresa_id = $1 AND codigo = $2`,
      [empresaId, sku],
    );
    await client.query(
      'INSERT INTO catalog.reservas_ordenes (order_id, items_json) VALUES ($1, $2)',
      [orderId, JSON.stringify([{ empresa_id: empresaId, sku, cantidad: 1 }])],
    );
    await assertStock(client, empresaId, sku, 9);
    // El stock de la otra empresa no se contamina (ADR-27).
    await assertStock(client, empresaOtra, sku, 4);

    const { rows: [reserva] } = await client.query(
      'SELECT items_json FROM catalog.reservas_ordenes WHERE order_id = $1',
      [orderId],
    );
    assert.equal(reserva.items_json[0].cantidad, 1);

    await client.query(
      'UPDATE catalog.items SET stock = stock + 1 WHERE empresa_id = $1 AND codigo = $2',
      [empresaId, sku],
    );
    await client.query('DELETE FROM catalog.reservas_ordenes WHERE order_id = $1', [orderId]);
    await assertStock(client, empresaId, sku, 10);
    await client.query('ROLLBACK');
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});

test('01e_adr25_empresas.sql deja el esquema que consume commissions-service', async () => {
  const { rows: columnas } = await client.query(
    `SELECT table_name, column_name
     FROM information_schema.columns
     WHERE table_schema = 'commissions'
       AND (
         (table_name = 'comisiones' AND column_name IN
           ('linea_clave', 'empresa_id', 'tienda_id', 'monto_empresario_cents', 'participacion_porcentaje'))
         OR
         (table_name = 'compensaciones_devoluciones' AND column_name IN
           ('empresa_id', 'monto_empresario_cents'))
       )
     ORDER BY table_name, column_name`,
  );
  assert.deepEqual(columnas, [
    { table_name: 'comisiones', column_name: 'empresa_id' },
    { table_name: 'comisiones', column_name: 'linea_clave' },
    { table_name: 'comisiones', column_name: 'monto_empresario_cents' },
    { table_name: 'comisiones', column_name: 'participacion_porcentaje' },
    { table_name: 'comisiones', column_name: 'tienda_id' },
    { table_name: 'compensaciones_devoluciones', column_name: 'empresa_id' },
    { table_name: 'compensaciones_devoluciones', column_name: 'monto_empresario_cents' },
  ]);

  const { rows: [tabla] } = await client.query(
    `SELECT to_regclass('commissions.liquidaciones_empresas') AS nombre`,
  );
  assert.equal(tabla.nombre, 'commissions.liquidaciones_empresas');

  const { rows: indices } = await client.query(
    `SELECT indexname FROM pg_indexes
     WHERE schemaname = 'commissions' AND indexname = 'idx_comisiones_orden_linea'`,
  );
  assert.deepEqual(indices, [{ indexname: 'idx_comisiones_orden_linea' }]);
});

async function assertStock(client, empresaId, sku, esperado) {
  const { rows: [row] } = await client.query(
    `SELECT stock FROM catalog.items
      WHERE empresa_id = $1 AND codigo = $2 AND tipo = 'producto'
        AND eliminado_en IS NULL`,
    [empresaId, sku],
  );
  assert.equal(row.stock, esperado);
}
