import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import pg from 'pg';

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

test('reserva, reconstruccion y devolucion conservan stock disponible exacto', async () => {
  const client = new pg.Client({ connectionString });
  await client.connect();
  try {
    for (const name of [
      '01_esquemas.sql',
      '01c_cqrs_catalog.sql',
      '01e_adr25_empresas.sql',
      '01f_slugs.sql',
      '01h_catalogo_por_empresa.sql',
      '01i_variantes_imagenes.sql',
    ]) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
    await client.query('BEGIN');
    try {
      const sku = `REG-${randomUUID()}`;
      const orderId = randomUUID();
      const empresaId = randomUUID();
      const empresaOtra = randomUUID();
      await client.query(
        `INSERT INTO catalog.productos (empresa_id, sku, nombre, precio_base_cents, stock)
         VALUES ($1, $2, 'Producto de regresion', 1000, 10), ($3, $2, 'Mismo SKU en otra empresa', 2000, 4)`,
        [empresaId, sku, empresaOtra],
      );
      await assertStock(client, empresaId, sku, { maestro: 10, fisico: 10, reservado: 0, disponible: 10 });
      await assertStock(client, empresaOtra, sku, { maestro: 4, fisico: 4, reservado: 0, disponible: 4 });

      await client.query(
        'UPDATE catalog.productos SET stock = stock - 1 WHERE empresa_id = $1 AND sku = $2',
        [empresaId, sku],
      );
      await client.query(
        'INSERT INTO catalog.reservas_ordenes (order_id, items_json) VALUES ($1, $2)',
        [orderId, JSON.stringify([{ empresa_id: empresaId, sku, cantidad: 1 }])],
      );
      await assertStock(client, empresaId, sku, { maestro: 9, fisico: 10, reservado: 1, disponible: 9 });
      // El stock de la otra empresa no se contamina (ADR-27).
      await assertStock(client, empresaOtra, sku, { maestro: 4, fisico: 4, reservado: 0, disponible: 4 });

      await client.query(
        'UPDATE catalog.reservas_ordenes SET items_json = $2 WHERE order_id = $1',
        [orderId, JSON.stringify([{ empresa_id: empresaId, sku, cantidad: 2 }])],
      );
      await assertStock(client, empresaId, sku, { maestro: 9, fisico: 11, reservado: 2, disponible: 9 });
      await client.query(
        'UPDATE catalog.reservas_ordenes SET items_json = $2 WHERE order_id = $1',
        [orderId, JSON.stringify([{ empresa_id: empresaId, sku, cantidad: 1 }])],
      );
      await assertStock(client, empresaId, sku, { maestro: 9, fisico: 10, reservado: 1, disponible: 9 });

      await client.query('SELECT catalog.fn_bootstrap_proyecciones()');
      await assertStock(client, empresaId, sku, { maestro: 9, fisico: 10, reservado: 1, disponible: 9 });

      await client.query(
        'UPDATE catalog.productos SET stock = stock + 1 WHERE empresa_id = $1 AND sku = $2',
        [empresaId, sku],
      );
      await client.query('DELETE FROM catalog.reservas_ordenes WHERE order_id = $1', [orderId]);
      await assertStock(client, empresaId, sku, { maestro: 10, fisico: 10, reservado: 0, disponible: 10 });
      await client.query('ROLLBACK');
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    }
  } finally {
    await client.end();
  }
});

async function assertStock(client, empresaId, sku, esperado) {
  const { rows: [row] } = await client.query(
    `SELECT p.stock AS maestro, s.stock_fisico AS fisico, s.reservado,
            s.disponible, c.disponible AS catalogo
     FROM catalog.productos p
     JOIN catalog.stock_vista s ON s.empresa_id = p.empresa_id AND s.sku = p.sku
     JOIN catalog.producto_catalogo_vista c ON c.empresa_id = p.empresa_id AND c.sku = p.sku
     WHERE p.empresa_id = $1 AND p.sku = $2`,
    [empresaId, sku],
  );
  assert.deepEqual(row, { ...esperado, catalogo: esperado.disponible });
}
