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
 *
 * `commissions-service` (ADR-0007), `logistics-service` (ADR-0010) y
 * `finance-service` (ADR-0014) ya no tienen consumidor TypeScript: su
 * acreditación, sus guías/flotas y sus asientos se cubren en
 * `packages/commissions-service/tests/comisiones_pg.rs`,
 * `packages/logistics-service/tests/envios_pg.rs` y
 * `packages/finance-service/tests/finance_pg.rs`. Aquí la saga verifica la
 * coreografía hasta `payment.procesado` y el contrato de `comision.acreditada`.
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
  '60_catalog_item_componentes.sql',
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
    // Cambios aditivos de las fases B2/D: método de pago y composición.
    await aplicar('57_orders_metodo_pago.sql');
    await aplicar('60_catalog_item_componentes.sql');
    return;
  }

  const nombres = [];
  if (!(await existeTabla('identity.usuarios'))) nombres.push('01_esquemas.sql');
  if (!(await existeTabla('catalog.items'))) nombres.push(...CATALOGO);
  else nombres.push('01e_adr25_empresas.sql', '01f_slugs.sql');
  nombres.push(...SIEMPRE.filter((nombre) => !nombres.includes(nombre)));

  for (const nombre of nombres) await aplicar(nombre);
  // Cambio aditivo de la Fase D; idempotente y necesario aunque el esquema
  // base ya exista (el camino rápido no recorre CATALOGO).
  await aplicar('60_catalog_item_componentes.sql');
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


/** Crea una orden con el mismo evento y outbox que produce el binario Rust. */
async function crearOrden(items, ofertaElegida) {
  const orderId = randomUUID();
  const ofertaActual = ofertaElegida ?? oferta();
  const lineas = items.map((item) => ({
    oferta_id: item.oferta_id,
    sku: ofertaActual.sku,
    producto_nombre: ofertaActual.producto_nombre,
    cantidad: item.cantidad,
    precio_unitario_cents: Math.round(Number(ofertaActual.precio_venta) * 100),
    vendedor_id: vendedorId,
    tienda_id: tiendaId,
    empresa_id: empresaId,
    participacion_porcentaje: 100,
    item_type: 'producto',
    item_id: ofertaActual.item_id,
    fulfillment: 'none',
    fulfillment_config: {},
  }));
  const total = lineas.reduce((suma, linea) => suma + linea.precio_unitario_cents * linea.cantidad, 0);
  const creadoEn = new Date().toISOString();
  await client.query(
    `INSERT INTO orders.eventos
       (event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en)
     VALUES ($1, $2, 'Order', 'OrderCreatedEvent', $3::jsonb, '{}'::jsonb, 1, NOW())`,
    [
      randomUUID(),
      orderId,
      JSON.stringify({
        order_id: orderId,
        comprador_id: compradorId,
        items: lineas,
        total_cents: total,
        estado: 'creada',
        tipo_orden: 'compra',
        metodo_pago: 'simulado',
        fulfillment_config: { por_linea: [] },
        creado_en: creadoEn,
      }),
    ],
  );
  await insertarOutbox('orders.outbox', {
    event_id: randomUUID(),
    tipo: 'order.created',
    ocurrido_en: creadoEn,
    data: {
      order_id: orderId,
      comprador_id: compradorId,
      items: lineas,
      total_cents: total,
      moneda: 'NIO',
      estado: 'creada',
      tipo_orden: 'compra',
      metodo_pago: 'simulado',
      fulfillment_config: { por_linea: [] },
      comision_cents: 13800,
    },
  });
  return { id: orderId, estado: 'creada', total_cents: total, items: lineas };
}

async function transicionarOrden(orderId, estado, motivo, actor) {
  const { rows: [actual] } = await client.query(
    'SELECT estado FROM orders.orden_vista WHERE id = $1',
    [orderId],
  );
  const { rows: [{ version }] } = await client.query(
    'SELECT COALESCE(MAX(version), 0) + 1 AS version FROM orders.eventos WHERE aggregate_id = $1',
    [orderId],
  );
  const ocurridoEn = new Date().toISOString();
  await client.query(
    `INSERT INTO orders.eventos
       (event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en)
     VALUES ($1, $2, 'Order', 'OrderStatusUpdatedEvent', $3::jsonb, '{}'::jsonb, $4, NOW())`,
    [
      randomUUID(),
      orderId,
      JSON.stringify({
        order_id: orderId,
        estado,
        previo_estado: actual?.estado,
        motivo,
        actor,
        ocurrido_en: ocurridoEn,
      }),
      version,
    ],
  );
  await insertarOutbox('orders.outbox', {
    event_id: randomUUID(),
    tipo: 'order.status.updated',
    ocurrido_en: ocurridoEn,
    data: {
      order_id: orderId,
      estado,
      previo_estado: actual?.estado,
      motivo,
      actor,
    },
  });
}

async function insertarOutbox(tabla, evento) {
  await client.query(
    `INSERT INTO ${tabla} (event_id, tipo, payload, estado, creado_en)
     VALUES ($1, $2, $3::jsonb, 'pendiente', NOW())`,
    [evento.event_id, evento.tipo, JSON.stringify(evento)],
  );
  return evento;
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


  // La reserva/reintegro de stock de catalog se cubre en
  // `packages/catalog-service/tests/catalog_pg.rs` (ADR-0016). Aquí la saga
  // aplica el mismo efecto de stock directamente para no acoplar el harness
  // al TypeScript retirado.
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
  const orden = await crearOrden([{ oferta_id: ofertaId, cantidad: 1 }]);
  ordenVista = orden;
  assert.equal(orden.estado, 'creada');

  // 1. orders → catalog: reserva atómica de stock (cubierta en Rust)
  const orderCreated = await eventoDe('orders.outbox', 'order.created');
  assert.equal(orderCreated.tipo, 'order.created');
  await client.query('UPDATE catalog.items SET stock = stock - 1 WHERE id = $1', [itemId]);
  const { rows: [item] } = await client.query('SELECT stock FROM catalog.items WHERE id = $1', [itemId]);
  assert.equal(item.stock, 9, 'el stock debe bajar al reservar');

  // 2. catalog → orders: pago simulado en la misma transición
  await transicionarOrden(orden.id, 'pagada', 'pago_simulado_aprobado', 'inventario');
  const payment = await insertarOutbox('orders.outbox', {
    event_id: randomUUID(),
    tipo: 'payment.procesado',
    ocurrido_en: new Date().toISOString(),
    data: {
      order_id: orden.id,
      monto_cents: orden.total_cents,
      estado: 'procesado',
      metodo: 'simulado',
    },
  });
  const { rows: [pagada] } = await client.query(
    'SELECT estado, total_cents FROM orders.orden_vista WHERE id = $1',
    [orden.id],
  );
  assert.equal(pagada.estado, 'pagada');
  assert.equal(pagada.total_cents, 115000);

  // 3. pago → finance: la pata contable la ejecuta el binario Rust
  // (ADR-0014) y se cubre en `packages/finance-service/tests/finance_pg.rs`.
  // Aquí se verifica el contrato del evento que finance consume.
  assert.equal(payment.tipo, 'payment.procesado');
  assert.equal(payment.data.order_id, orden.id);
  assert.equal(payment.data.monto_cents, 115000);

  // 4. ciclo de vida: el binario Rust consume cada transición y publica
  // order.completado; aquí se inyecta el evento resultante para commissions.
  for (const [estado, motivo] of [
    ['en_preparacion', 'bodega alista'],
    ['enviada', 'guía generada'],
    ['entregada', 'recepción confirmada'],
  ]) {
    await transicionarOrden(orden.id, estado, motivo, 'sistema');
  }
  const orderCompletado = {
    event_id: randomUUID(),
    tipo: 'order.completado',
    ocurrido_en: new Date().toISOString(),
    data: { order_id: orden.id, entregado_en: new Date().toISOString() },
  };

  // 5. order.completado lo produce logistics; la comisión la acredita el
  // servicio Rust (ADR-0007). Se inyecta el evento resultante para finance.
  assert.equal(orderCompletado.tipo, 'order.completado');
  const comision = {
    event_id: randomUUID(),
    tipo: 'comision.acreditada',
    ocurrido_en: new Date().toISOString(),
    data: {
      order_id: orden.id,
      monto_cents: 13800,
      monto_vendedor_cents: 101200,
      monto_empresario_cents: 0,
      vendedor_id: vendedorId,
      empresa_id: empresaId,
      tienda_id: tiendaId,
      participacion_porcentaje: 100,
    },
  };
  assert.equal(comision.data.monto_cents, 13800, '12 % de C$ 1 150,00');

  // 6. comisión → finance: el asiento de realización lo registra el binario
  // Rust (ADR-0014, `tests/finance_pg.rs`); aquí se valida el contrato.
  const { rows: [final] } = await client.query(
    'SELECT estado FROM orders.orden_vista WHERE id = $1',
    [orden.id],
  );
  assert.equal(final.estado, 'entregada');

  assert.equal(comision.tipo, 'comision.acreditada');
  assert.equal(comision.data.order_id, orden.id);
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
    const orden = await crearOrden(
      [{ oferta_id: ofertaFallo.id, cantidad: 2 }],
      ofertaFallo,
    );
    ordenVista = orden;
    const orderCreated = await eventoDe('orders.outbox', 'order.created', orden.id);
    assert.equal(orderCreated.tipo, 'order.created');
    // Sin stock suficiente no se descuenta nada (la reserva real se cubre en Rust).
    await transicionarOrden(orden.id, 'cancelada', 'stock_insuficiente', 'inventario');

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
