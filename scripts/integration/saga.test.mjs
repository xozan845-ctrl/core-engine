import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/* global Response */

/**
 * Saga completa (G-4): order.created → stock.reservado → payment.procesado →
 * shipment.started → order.completado → comision.acreditada → asiento.registrado.
 *
 * No levanta el stack ni RabbitMQ: usa los consumidores y handlers REALES
 * compilados contra PostgreSQL real y un broker falso. El outbox de cada
 * servicio es la fuente de verdad del paso anterior, así que la prueba verifica
 * la coreografía y la persistencia, no el transporte AMQP (eso queda en E2E).
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

process.env.DATABASE_URL = connectionString;
process.env.COMMISSION_RATE = '0.12';
process.env.PLATFORM_LOGISTICS_TENANT_ID = '00000000-0000-0000-0000-000000000001';

const CATALOGO = [
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
const SIEMPRE = [
  '01b_cqrs_finance.sql',
  '01d_es_orders.sql',
  '01e_adr25_empresas.sql',
  '01f_slugs.sql',
  '27_orders_types.sql',
  '45_hardening.sql',
  '46_commission_payload.sql',
  '47_orders_comprador_sin_comisiones.sql',
  '57_orders_metodo_pago.sql',
];

const rabbit = {
  declararColas: async () => {},
  consumir: async () => {},
  activarReintento: () => {},
  publicar: async () => true,
};

let client;
let pgService;
let handler;
let catalogConsumer;
let ordersConsumer;
let logisticsConsumer;
let commissionsConsumer;
let financeConsumer;

let empresaId;
let tiendaId;
let vendedorId;
let compradorId;
let itemId;
let ofertaId;
let sku;
let ordenVista;

const oferta = () => ({
  id: ofertaId,
  tienda_id: tiendaId,
  vendedor_id: vendedorId,
  empresa_id: empresaId,
  participacion_porcentaje: 100,
  item_type: 'producto',
  item_id: itemId,
  sku,
  producto_nombre: 'Producto de la saga',
  margen: 15,
  precio_base: '1000.00',
  precio_venta: '1150.00',
  stock: 10,
  estado: 'activa',
});

function responder(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

async function aplicarMigraciones() {
  const sonda = new pg.Client({ connectionString });
  await sonda.connect();
  let completo;
  try {
    const { rows: [{ existe }] } = await sonda.query(
      "SELECT to_regnamespace('identity') IS NOT NULL AS existe",
    );
    const { rows: [{ final }] } = await sonda.query(
      "SELECT to_regclass('catalog.items') IS NOT NULL AND to_regclass('catalog.productos') IS NULL AS final",
    );
    const { rows: [{ tablas }] } = await sonda.query(
      `SELECT to_regclass('orders.orden_vista') IS NOT NULL
          AND to_regclass('commissions.comisiones') IS NOT NULL
          AND to_regclass('logistics.envios') IS NOT NULL
          AND to_regclass('finance.asientos') IS NOT NULL AS tablas`,
    );
    completo = existe && final && tablas;
  } finally {
    await sonda.end();
  }
  if (completo) {
    // 01b es idempotente y reemplaza la función financiera legacy ambigua que
    // pudo quedar de una cadena anterior. No se reaplican 01d/01e/27/45/46/47:
    // no son idempotentes sobre un esquema ya evolucionado.
    await aplicar('01b_cqrs_finance.sql');
    // Cambio aditivo de la Fase B2: la proyección necesita metodo_pago.
    await aplicar('57_orders_metodo_pago.sql');
    return;
  }

  const nombres = [];
  if (!(await existeTabla('identity.usuarios'))) nombres.push('01_esquemas.sql');
  if (!(await existeTabla('catalog.items'))) nombres.push(...CATALOGO);
  else nombres.push('01e_adr25_empresas.sql', '01f_slugs.sql');
  nombres.push(...SIEMPRE.filter((nombre) => !nombres.includes(nombre)));

  for (const nombre of nombres) await aplicar(nombre);
}

async function aplicar(nombre) {
  const conexion = new pg.Client({ connectionString });
  await conexion.connect();
  try {
    const sql = await readFile(new URL(`../../infra/db/init/${nombre}`, import.meta.url), 'utf8');
    await conexion.query(sql);
  } catch (error) {
    console.error(`[saga] fallo la migración ${nombre}: ${error.message}`);
    throw error;
  } finally {
    await conexion.end();
  }
}

async function existeTabla(nombre) {
  const sonda = new pg.Client({ connectionString });
  await sonda.connect();
  try {
    const { rows: [{ existe }] } = await sonda.query('SELECT to_regclass($1) IS NOT NULL AS existe', [nombre]);
    return existe;
  } finally {
    await sonda.end();
  }
}

async function eventoDe(tabla, tipo, orderId) {
  const filtro = orderId ? " AND payload->'data'->>'order_id' = $2" : '';
  const parametros = orderId ? [tipo, orderId] : [tipo];
  const { rows } = await client.query(
    `SELECT payload FROM ${tabla} WHERE tipo = $1${filtro} ORDER BY creado_en DESC, ctid DESC LIMIT 1`,
    parametros,
  );
  assert.ok(rows[0], `falta el evento ${tipo} en ${tabla}`);
  return rows[0].payload;
}

async function eventosDe(tabla, tipo) {
  const { rows } = await client.query(
    `SELECT payload FROM ${tabla} WHERE tipo = $1 ORDER BY creado_en ASC`,
    [tipo],
  );
  return rows.map((fila) => fila.payload);
}

async function crearItem(codigo, stock) {
  const { rows: [fila] } = await client.query(
    `INSERT INTO catalog.items
       (empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock)
     VALUES ($1, 'producto', $2, $3, 1000, true, $4)
     RETURNING id`,
    [empresaId, codigo, `Producto ${codigo}`, stock],
  );
  return fila.id;
}

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    await aplicarMigraciones();
    // 36_partes_compatibilidad añade esta columna; la suite se centra en la
    // saga y no aplica la cadena de partes completa.
    await client.query('ALTER TABLE orders.orden_vista ADD COLUMN IF NOT EXISTS parte_id uuid');
    await client.query('ALTER TABLE logistics.envios ADD COLUMN IF NOT EXISTS parte_id uuid');
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  empresaId = randomUUID();
  tiendaId = randomUUID();
  vendedorId = randomUUID();
  compradorId = randomUUID();
  ofertaId = randomUUID();
  sku = `SAGA-${randomUUID()}`;
  itemId = await crearItem(sku, 10);

  const shared = await import('@core/shared');
  const { PgService, OutboxService } = shared;
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();

  const outboxCatalog = new OutboxService(pgService, rabbit, 'catalog.outbox');
  const outboxLogistics = new OutboxService(pgService, rabbit, 'logistics.outbox');
  const outboxCommissions = new OutboxService(pgService, rabbit, 'commissions.outbox');
  const outboxFinance = new OutboxService(pgService, rabbit, 'finance.outbox');

  const [
    { CreateOrderCommandHandler },
    { OrderEventStore },
    { OrderViewRepository },
    { CarritoService },
    { PedidosConsumer: OrdersConsumer },
    { ProductosService },
    { PedidosConsumer: CatalogConsumer },
    { EnviosService },
    { PedidosConsumer: LogisticsConsumer },
    { ComisionesService },
    { PedidosConsumer: CommissionsConsumer },
    { EventsConsumer: FinanceConsumer },
    { ContabilidadService },
    { MetricasService },
  ] = await Promise.all([
    import('../../packages/orders-service/dist/pedidos/handlers/create-order-command.handler.js'),
    import('../../packages/orders-service/dist/pedidos/repositories/order-event-store.js'),
    import('../../packages/orders-service/dist/pedidos/queries/order-view.repository.js'),
    import('../../packages/orders-service/dist/pedidos/carrito/carrito.service.js'),
    import('../../packages/orders-service/dist/pedidos/pedidos.consumer.js'),
    import('../../packages/catalog-service/dist/productos/productos.service.js'),
    import('../../packages/catalog-service/dist/eventos/pedidos.consumer.js'),
    import('../../packages/logistics-service/dist/envios/envios.service.js'),
    import('../../packages/logistics-service/dist/envios/pedidos.consumer.js'),
    import('../../packages/commissions-service/dist/comisiones/comisiones.service.js'),
    import('../../packages/commissions-service/dist/comisiones/pedidos.consumer.js'),
    import('../../packages/finance-service/dist/eventos/events.consumer.js'),
    import('../../packages/finance-service/dist/contabilidad/contabilidad.service.js'),
    import('../../packages/finance-service/dist/eventos/metricas.service.js'),
  ]);

  handler = new CreateOrderCommandHandler(
    pgService,
    new OrderEventStore(pgService),
    new OrderViewRepository(pgService),
    new CarritoService(pgService),
  );
  catalogConsumer = new CatalogConsumer(
    rabbit,
    outboxCatalog,
    pgService,
    new ProductosService(pgService, outboxCatalog),
  );
  ordersConsumer = new OrdersConsumer(rabbit, pgService, handler);
  logisticsConsumer = new LogisticsConsumer(
    rabbit,
    outboxLogistics,
    pgService,
    new EnviosService(pgService),
  );
  commissionsConsumer = new CommissionsConsumer(
    rabbit,
    outboxCommissions,
    pgService,
    new ComisionesService(pgService),
  );
  financeConsumer = new FinanceConsumer(
    rabbit,
    pgService,
    new ContabilidadService(pgService, outboxFinance),
    new MetricasService(pgService),
  );

  globalThis.fetch = async (url) => {
    const destino = String(url);
    if (destino.includes('/internal/ofertas')) return responder([oferta()]);
    if (destino.includes('/internal/items/lote')) {
      return responder([{ empresa_id: empresaId, sku, stock: 10, estado: 'activo' }]);
    }
    if (destino.includes('/internal/empresas/capacidades')) {
      return responder({ capacidades: [] });
    }
    if (destino.includes('/internal/orders/')) {
      return responder({
        total_cents: ordenVista?.total_cents ?? 0,
        parte_id: null,
        items: ordenVista?.items ?? [],
      });
    }
    return responder({}, 404);
  };
});

after(async () => {
  await pgService?.onModuleDestroy();
  await client?.end();
});

test('saga completa: orden → stock → pago → envío → comisión → asiento', { concurrency: false }, async () => {
  const orden = await handler.ejecutar(
    { tipo_orden: 'compra', items: [{ oferta_id: ofertaId, cantidad: 1 }] },
    compradorId,
  );
  ordenVista = orden;
  assert.equal(orden.estado, 'creada');

  // 1. orders → catalog: reserva atómica de stock
  const orderCreated = await eventoDe('orders.outbox', 'order.created');
  await catalogConsumer['manejar'](orderCreated);
  const stockReservado = await eventoDe('catalog.outbox', 'stock.reservado');
  const { rows: [item] } = await client.query('SELECT stock FROM catalog.items WHERE id = $1', [itemId]);
  assert.equal(item.stock, 9, 'el stock debe bajar al reservar');

  // 2. catalog → orders: pago simulado en la misma transición
  await ordersConsumer['manejar'](stockReservado);
  const { rows: [pagada] } = await client.query(
    'SELECT estado, total_cents FROM orders.orden_vista WHERE id = $1',
    [orden.id],
  );
  assert.equal(pagada.estado, 'pagada');
  assert.equal(pagada.total_cents, 115000);
  const payment = await eventoDe('orders.outbox', 'payment.procesado');

  // 3. pago → logistics (envío), commissions (pago) y finance (asiento de caja)
  await logisticsConsumer['manejar'](payment);
  await commissionsConsumer['manejar'](payment);
  await financeConsumer['manejar'](orderCreated);
  await financeConsumer['manejar'](payment);

  const { rows: envios } = await client.query(
    'SELECT guia, estado FROM logistics.envios WHERE order_id = $1',
    [orden.id],
  );
  assert.equal(envios.length, 1, 'logistics debe crear un envío');
  assert.equal(envios[0].estado, 'en_preparacion');
  assert.match(envios[0].guia, /^BGH-/);

  const { rows: asientosPago } = await client.query(
    'SELECT id FROM finance.asientos WHERE referencia_id = $1',
    [orden.id],
  );
  assert.equal(asientosPago.length, 1, 'finance debe registrar el asiento del pago');

  // 4. ciclo de vida: logistics consume cada transición y publica order.completado
  for (const [estado, motivo] of [
    ['en_preparacion', 'bodega alista'],
    ['enviada', 'guía generada'],
    ['entregada', 'recepción confirmada'],
  ]) {
    await handler.transicionar(orden.id, estado, motivo, 'sistema');
  }
  for (const evento of await eventosDe('orders.outbox', 'order.status.updated')) {
    await logisticsConsumer['manejar'](evento);
  }
  const orderCompletado = await eventoDe('logistics.outbox', 'order.completado');

  // 5. order.completado → commissions → comision.acreditada
  await commissionsConsumer['manejar'](orderCompletado);
  const comision = await eventoDe('commissions.outbox', 'comision.acreditada');
  assert.equal(comision.data.monto_cents, 13800, '12 % de C$ 1 150,00');

  // 6. comisión → finance: realización del ingreso y asiento de cierre
  await financeConsumer['manejar'](comision);

  const { rows: [final] } = await client.query(
    'SELECT estado FROM orders.orden_vista WHERE id = $1',
    [orden.id],
  );
  assert.equal(final.estado, 'entregada');

  const { rows: comisiones } = await client.query(
    'SELECT comision_cents, monto_vendedor_cents, estado FROM commissions.comisiones WHERE order_id = $1',
    [orden.id],
  );
  assert.equal(comisiones.length, 1);
  assert.equal(comisiones[0].comision_cents, 13800);
  assert.equal(comisiones[0].monto_vendedor_cents, 101200);
  assert.equal(comisiones[0].estado, 'acreditada');

  const { rows: asientos } = await client.query(
    'SELECT tipo, referencia_tipo FROM finance.asientos WHERE referencia_id = $1 ORDER BY creado_en',
    [orden.id],
  );
  assert.equal(asientos.length, 2, 'pago + comisión deben producir dos asientos');
  assert.deepEqual(asientos.map((a) => a.tipo), ['INGRESO', 'AJUSTE']);

  const { rows: asientosOutbox } = await client.query(
    `SELECT payload FROM finance.outbox
      WHERE tipo = 'asiento.registrado'
        AND payload->'data'->>'referencia_id' = $1`,
    [orden.id],
  );
  assert.equal(asientosOutbox.length, 2, 'cada asiento publica asiento.registrado');

  const { rows: procesados } = await client.query(
    `SELECT tipo FROM finance.eventos_procesados
      WHERE event_id IN ($1, $2) ORDER BY tipo`,
    [payment.event_id, comision.event_id],
  );
  assert.deepEqual(procesados.map((p) => p.tipo).sort(), ['comision.acreditada', 'payment.procesado']);
});

test('stock.fallido compensa la orden a cancelada sin descontar stock', { concurrency: false }, async () => {
  const codigo = `SAGA-FALLO-${randomUUID()}`;
  const itemFallo = await crearItem(codigo, 1);
  const ofertaFallo = {
    ...oferta(),
    id: randomUUID(),
    item_id: itemFallo,
    sku: codigo,
  };
  const fetchOriginal = globalThis.fetch;
  globalThis.fetch = async (url) => {
    const destino = String(url);
    if (destino.includes('/internal/ofertas')) return responder([ofertaFallo]);
    if (destino.includes('/internal/items/lote')) {
      return responder([{ empresa_id: empresaId, sku: codigo, stock: 10, estado: 'activo' }]);
    }
    return fetchOriginal(url);
  };

  try {
    const orden = await handler.ejecutar(
      { tipo_orden: 'compra', items: [{ oferta_id: ofertaFallo.id, cantidad: 2 }] },
      compradorId,
    );
    ordenVista = orden;
    const orderCreated = await eventoDe('orders.outbox', 'order.created', orden.id);
    await catalogConsumer['manejar'](orderCreated);
    const stockFallido = await eventoDe('catalog.outbox', 'stock.fallido', orden.id);
    await ordersConsumer['manejar'](stockFallido);

    const { rows: [cancelada] } = await client.query(
      'SELECT estado FROM orders.orden_vista WHERE id = $1',
      [orden.id],
    );
    assert.equal(cancelada.estado, 'cancelada');
    const { rows: [item] } = await client.query('SELECT stock FROM catalog.items WHERE id = $1', [itemFallo]);
    assert.equal(item.stock, 1, 'el stock no cambia si la reserva falla');
  } finally {
    globalThis.fetch = fetchOriginal;
  }
});
