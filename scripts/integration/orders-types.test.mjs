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

const MIGRACIONES = ['01_esquemas.sql', '01d_es_orders.sql', '27_orders_types.sql', '45_hardening.sql', '46_commission_payload.sql'];
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

test('proyecta tipo de orden y fulfillment por línea sin romper compras legacy', async () => {
  const orderId = randomUUID();
  const vendedorId = randomUUID();
  const compradorId = randomUUID();
  const tiendaId = randomUUID();
  const creadoEn = new Date().toISOString();
  const payload = {
    order_id: orderId,
    cliente_id: compradorId,
    tipo_orden: 'orden_servicio',
    fulfillment_config: { por_linea: [{ oferta_id: 'of-1', fulfillment: 'onsite' }] },
    items: [{
      oferta_id: 'of-1',
      item_type: 'servicio',
      item_id: randomUUID(),
      sku: 'SERV-001',
      producto_nombre: 'Instalación',
      cantidad: 1,
      precio_unitario_cents: 250000,
      vendedor_id: vendedorId,
      tienda_id: tiendaId,
      empresa_id: randomUUID(),
      participacion_porcentaje: 25,
      fulfillment: 'onsite',
    }],
    total_cents: 250000,
    estado: 'creada',
    creado_en: creadoEn,
  };
  await client.query('SELECT pg_advisory_lock(38194201)');
  await client.query('BEGIN');
  try {
    await client.query(
      `INSERT INTO orders.eventos (aggregate_id, aggregate_type, tipo, payload, version, creado_en)
       VALUES ($1, 'Order', 'OrderCreatedEvent', $2::jsonb, 1, $3)`,
      [orderId, JSON.stringify(payload), creadoEn],
    );
    const { rows: [vista] } = await client.query(
      `SELECT tipo_orden, fulfillment_config, items_json
         FROM orders.orden_vista WHERE id = $1`,
      [orderId],
    );
    assert.equal(vista.tipo_orden, 'orden_servicio');
    assert.deepEqual(vista.fulfillment_config, payload.fulfillment_config);
    assert.equal(vista.items_json[0].fulfillment, 'onsite');
    await client.query('ROLLBACK');
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
