import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Consumo FEFO de lotes (R-I-12, farmacia).
 * El lote que vence antes se consume primero; una cantidad insuficiente
 * revierte toda la transacción.
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

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
];

let client;
let inventario;
let productos;
let pgService;
let empresaId;
let itemId;
let itemServicioId;
let itemOrdenId;
let skuOrden;

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    const { rows: [{ final }] } = await client.query(
      `SELECT to_regclass('catalog.items') IS NOT NULL
              AND to_regclass('catalog.productos') IS NULL AS final`,
    );
    if (final) {
      const sql = await readFile(new URL('../../infra/db/init/53_reservas_lotes.sql', import.meta.url), 'utf8');
      await client.query(sql);
    } else {
      const { rows: [{ existe }] } = await client.query(
        `SELECT to_regnamespace('identity') IS NOT NULL AS existe`,
      );
      const migraciones = existe ? MIGRACIONES.filter((name) => name !== '01_esquemas.sql') : MIGRACIONES;
      for (const name of migraciones) {
        const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
        await client.query(sql);
      }
    }
    empresaId = randomUUID();
    const { rows: [item] } = await client.query(
      `INSERT INTO catalog.items
         (empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock)
       VALUES ($1, 'producto', $2, 'Medicamento FEFO', 1000, true, 30)
       RETURNING id`,
      [empresaId, `FEFO-${randomUUID().slice(0, 8)}`],
    );
    itemId = item.id;
    const { rows: [servicio] } = await client.query(
      `INSERT INTO catalog.items
         (empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, duration_minutes)
       VALUES ($1, 'servicio', $2, 'Servicio sin stock', 1000, false, 30)
       RETURNING id`,
      [empresaId, `FEFO-S-${randomUUID().slice(0, 8)}`],
    );
    itemServicioId = servicio.id;
    await client.query(
      `INSERT INTO catalog.lotes_inventario
         (empresa_id, item_id, codigo_lote, vence_en, cantidad_inicial, cantidad_disponible)
       VALUES
         ($1, $2, 'L-TARDE', '2028-12-31', 5, 5),
         ($1, $2, 'L-PRONTO', '2027-06-30', 10, 10),
         ($1, $2, 'L-SIN-FECHA', NULL, 10, 10),
         ($1, $2, 'L-VENCIDO', '2020-01-01', 10, 10)`,
      [empresaId, itemId],
    );
    skuOrden = `ORDEN-FEFO-${randomUUID().slice(0, 8)}`;
    const { rows: [itemOrden] } = await client.query(
      `INSERT INTO catalog.items
         (empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock)
       VALUES ($1, 'producto', $2, 'Producto reserva FEFO', 1000, true, 10)
       RETURNING id`,
      [empresaId, skuOrden],
    );
    itemOrdenId = itemOrden.id;
    await client.query(
      `INSERT INTO catalog.lotes_inventario
         (empresa_id, item_id, codigo_lote, vence_en, cantidad_inicial, cantidad_disponible)
       VALUES
         ($1, $2, 'ORDEN-A', '2027-01-01', 5, 5),
         ($1, $2, 'ORDEN-B', '2028-01-01', 5, 5)`,
      [empresaId, itemOrdenId],
    );
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const [{ InventarioService }, { ProductosService }] = await Promise.all([
    import('../../packages/catalog-service/dist/inventario/inventario.service.js'),
    import('../../packages/catalog-service/dist/productos/productos.service.js'),
  ]);
  const shared = await import('@core/shared');
  pgService = new shared.PgService({ url: connectionString });
  await pgService.onModuleInit();
  inventario = new InventarioService(pgService);
  productos = new ProductosService(pgService);
});

after(async () => {
  await pgService?.onModuleDestroy();
  await client?.query('DELETE FROM catalog.lotes_inventario WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await client?.query('DELETE FROM catalog.items WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await client?.end();
});

test('consume primero el lote con vencimiento más próximo', async () => {
  const resultado = await inventario.consumirLotes({
    empresa_id: empresaId,
    item_id: itemId,
    cantidad: 8,
  });

  assert.deepEqual(resultado, {
    consumidos: [{ lote_id: (await lote('L-PRONTO')).id, codigo_lote: 'L-PRONTO', cantidad: 8 }],
    total_consumido: 8,
  });
  assert.equal((await lote('L-PRONTO')).cantidad_disponible, 2);
  assert.equal((await lote('L-TARDE')).cantidad_disponible, 5);
  assert.equal((await lote('L-VENCIDO')).cantidad_disponible, 10);
  assert.equal((await stock(itemId)), 22);
});

test('divide el consumo entre lotes siguiendo FEFO', async () => {
  const resultado = await inventario.consumirLotes({
    empresa_id: empresaId,
    item_id: itemId,
    cantidad: 4,
  });

  assert.deepEqual(resultado.consumidos.map((c) => [c.codigo_lote, c.cantidad]), [
    ['L-PRONTO', 2],
    ['L-TARDE', 2],
  ]);
});

test('una cantidad insuficiente revierte el consumo completo', async () => {
  const antes = await lote('L-TARDE');
  await assert.rejects(
    inventario.consumirLotes({ empresa_id: empresaId, item_id: itemId, cantidad: 999 }),
    (error) => error?.codigo === 'STOCK_LOTE_INSUFICIENTE',
  );
  assert.equal((await lote('L-TARDE')).cantidad_disponible, antes.cantidad_disponible);
});

test('no consume lotes vencidos y no descuenta stock si faltan', async () => {
  const antes = await lote('L-VENCIDO');
  await assert.rejects(
    inventario.consumirLotes({
      empresa_id: empresaId,
      item_id: itemId,
      cantidad: 999,
    }),
    (error) => error?.codigo === 'STOCK_LOTE_INSUFICIENTE',
  );
  assert.equal((await lote('L-VENCIDO')).cantidad_disponible, antes.cantidad_disponible);
});

test('la reserva de orden consume lotes y la devolucion los restaura', async () => {
  const orderId = randomUUID();
  const reservado = await productos.reservarStock(orderId, [
    { empresa_id: empresaId, sku: skuOrden, cantidad: 7 },
  ]);
  assert.equal(reservado.ok, true);
  assert.equal(await stock(itemOrdenId), 3);
  assert.equal((await loteOrden('ORDEN-A')).cantidad_disponible, 0);
  assert.equal((await loteOrden('ORDEN-B')).cantidad_disponible, 3);
  assert.equal((await reservasLotes(orderId)).length, 2);

  await productos.reintegrarStock(orderId, [
    { empresa_id: empresaId, sku: skuOrden, cantidad: 7 },
  ]);
  assert.equal(await stock(itemOrdenId), 10);
  assert.equal((await loteOrden('ORDEN-A')).cantidad_disponible, 5);
  assert.equal((await loteOrden('ORDEN-B')).cantidad_disponible, 5);
  assert.equal((await reservasLotes(orderId)).length, 0);
});

test('rechaza el consumo en items sin inventario', async () => {
  await assert.rejects(
    inventario.consumirLotes({ empresa_id: empresaId, item_id: itemServicioId, cantidad: 1 }),
    (error) => error?.codigo === 'INVENTARIO_NO_APLICA',
  );
});

async function stock(id) {
  const { rows: [fila] } = await client.query('SELECT stock FROM catalog.items WHERE id = $1', [id]);
  return fila.stock;
}

async function loteOrden(codigo) {
  const { rows: [fila] } = await client.query(
    `SELECT id, cantidad_disponible FROM catalog.lotes_inventario
      WHERE empresa_id = $1 AND item_id = $2 AND codigo_lote = $3`,
    [empresaId, itemOrdenId, codigo],
  );
  return fila;
}

async function reservasLotes(orderId) {
  const { rows } = await client.query(
    'SELECT lote_id, cantidad FROM catalog.reservas_lotes WHERE order_id = $1',
    [orderId],
  );
  return rows;
}

async function lote(codigo) {
  const { rows: [fila] } = await client.query(
    `SELECT id, cantidad_disponible FROM catalog.lotes_inventario
      WHERE empresa_id = $1 AND item_id = $2 AND codigo_lote = $3`,
    [empresaId, itemId, codigo],
  );
  return fila;
}
