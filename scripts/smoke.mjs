/**
 * Core Engine · Core Engine
 * Demo del flujo real (entregable cap. 7.1): alta de producto, oferta del
 * vendedor, compra, descuento de stock, orden, comision y liquidacion.
 *
 * Requiere el ecosistema levantado: docker compose up -d --build
 * Uso: node scripts/smoke.mjs
 */

const BASE = process.env.BASE_URL ?? 'http://localhost:8080';
const ADMIN_EMAIL = process.env.ADMIN_EMAIL ?? 'admin@core-engine.test';
const ADMIN_PASSWORD = process.env.ADMIN_PASSWORD ?? 'AdminCore Engine2026!';

let pasos = 0;
let fallidas = 0;

function paso(nombre) {
  pasos += 1;
  process.stdout.write(`\n[${String(pasos).padStart(2, '0')}] ${nombre} ... `);
}

function ok(detalle = '') {
  console.log(`OK ${detalle}`);
}

function fail(detalle = '') {
  fallidas += 1;
  console.log(`FAIL ${detalle}`);
}

async function api(metodo, ruta, cuerpo, token) {
  const res = await fetch(`${BASE}${ruta}`, {
    method: metodo,
    headers: {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: cuerpo ? JSON.stringify(cuerpo) : undefined,
  });
  const texto = await res.text();
  let json = null;
  try {
    json = texto ? JSON.parse(texto) : null;
  } catch {
    json = texto;
  }
  return { status: res.status, json };
}

async function esperar(fn, descripcion, intentos = 20, intervaloMs = 800) {
  let ultimo = null;
  for (let i = 0; i < intentos; i += 1) {
    ultimo = await fn();
    if (ultimo) return ultimo;
    await new Promise((r) => setTimeout(r, intervaloMs));
  }
  throw new Error(`Expiró la espera: ${descripcion} (último: ${JSON.stringify(ultimo)})`);
}

function sufijo() {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

async function main() {
  paso('Salud del ecosistema y aislado de la base');
  const salud = await api('GET', '/health');
  if (salud.status === 200 && salud.json?.api_gateway === 'ok') {
    ok(`gateway ok · servicios: ${Object.keys(salud.json.servicios ?? {}).length}`);
  } else {
    fail(`health=${salud.status}`);
    process.exit(1);
  }

  // ── 1. Identidad ──────────────────────────────────────────────────────
  const suf = sufijo();
  const correoVendedor = `vendedor.${suf}@core-engine.test`;
  const correoComprador = `comprador.${suf}@core-engine.test`;

  paso('Administrador: login (RF-01, seed del arranque)');
  const loginAdmin = await api('POST', '/api/v1/auth/login', {
    correo: ADMIN_EMAIL,
    contrasena: ADMIN_PASSWORD,
  });
  if (loginAdmin.status === 200 && loginAdmin.json?.access_token) {
    ok();
  } else {
    fail(`login admin → ${loginAdmin.status} ${JSON.stringify(loginAdmin.json)}`);
    process.exit(1);
  }
  const tokenAdmin = loginAdmin.json.access_token;

  paso('Registro de vendedor y comprador (RF-01)');
  const [regV, regC] = await Promise.all([
    api('POST', '/api/v1/auth/registro', {
      nombre: 'Vendedora Piloto',
      correo: correoVendedor,
      contrasena: 'Contrasena123!',
      rol: 'vendedor',
    }),
    api('POST', '/api/v1/auth/registro', {
      nombre: 'Comprador Piloto',
      correo: correoComprador,
      contrasena: 'Contrasena123!',
      rol: 'comprador',
    }),
  ]);
  if (regV.status === 201 || regV.status === 200) ok();
  if (regC.status === 201 || regC.status === 200) ok();
  else fail(`registro comprador → ${regC.status}`);
  const tokenVendedor = regV.json.access_token;
  const tokenComprador = regC.json.access_token;
  const idVendedor = regV.json.usuario.id;

  // ── 2. Catálogo (TC-01) ───────────────────────────────────────────────
  paso('Admin da de alta el producto (TC-01: stock 10, precio base C$ 1 000)');
  const crear = await api(
    'POST',
    '/api/v1/catalog/productos',
    { sku: `ACE-${suf.toUpperCase()}`, nombre: 'Aceite de cocina 1L', descripcion: 'Aceite vegetal', categoria: 'abarrotes', precio_base: '1000.00', stock: 10 },
    tokenAdmin,
  );
  if (crear.status === 201 || crear.status === 200) ok(`sku=${crear.json.sku}`);
  else {
    fail(`crear producto → ${crear.status} ${JSON.stringify(crear.json)}`);
    process.exit(1);
  }
  const producto = crear.json;

  paso('Tc-07a: /admin/reportes sin token → 401');
  const sinToken = await api('GET', '/api/v1/admin/reportes');
  if (sinToken.status === 401) ok();
  else fail(`esperaba 401, obtuve ${sinToken.status}`);

  // ── 3. Tienda y oferta (TC-02) ────────────────────────────────────────
  paso('Vendedor crea su tienda virtual (H-01)');
  const tienda = await api('POST', '/api/v1/vendedores/tienda', { nombre: 'Tienda de la Vendedora Piloto' }, tokenVendedor);
  if (tienda.status === 201 || tienda.status === 200) ok();
  else {
    fail(`tienda → ${tienda.status} ${JSON.stringify(tienda.json)}`);
    process.exit(1);
  }

  paso('Publica oferta con margen 15 % (TC-02: precio final = C$ 1 150,00)');
  const oferta = await api(
    'POST',
    '/api/v1/vendedores/productos',
    { producto_id: producto.id, margen: 15 },
    tokenVendedor,
  );
  if (oferta.status === 201 || oferta.status === 200) {
    const esperado = '1150.00';
    if (oferta.json.precio_venta === esperado) ok(`precio_venta=${oferta.json.precio_venta}`);
    else fail(`precio_venta=${oferta.json.precio_venta}, esperaba ${esperado}`);
  } else {
    fail(`oferta → ${oferta.status} ${JSON.stringify(oferta.json)}`);
    process.exit(1);
  }

  paso('Tienda pública visible para el comprador (H-02 checkout < 2 min)');
  const tiendaPublica = await api('GET', `/api/v1/tiendas/${tienda.json.id}`);
  if (tiendaPublica.status === 200 && tiendaPublica.json.ofertas.length === 1) ok();
  else fail(`tienda pública → ${tiendaPublica.status}`);

  // ── 4. Orden y saga de stock (TC-03, RN-03) ───────────────────────────
  paso('Comprador crea la orden (TC-03: checkout)');
  const orden = await api(
    'POST',
    '/api/v1/orders',
    { items: [{ oferta_id: oferta.json.id, cantidad: 1 }] },
    tokenComprador,
  );
  if (orden.status === 201 || orden.status === 200) ok(`order_id=${orden.json.id?.slice(0, 8)}`);
  else {
    fail(`orden → ${orden.status} ${JSON.stringify(orden.json)}`);
    process.exit(1);
  }
  const orderId = orden.json.id;

  paso('Saga asíncrona: stock reservado → orden Pagada + pago simulado');
  const estadoPagada = await esperar(
    async () => {
      const r = await api('GET', `/api/v1/orders/${orderId}`, null, tokenComprador);
      return r.json?.estado === 'pagada' ? r.json : null;
    },
    'orden pagada',
  );
  ok(`estado=${estadoPagada.estado} total=${estadoPagada.total}`);

  paso('TC-03: el stock del catálogo decreció');
  const catalogo = await api('GET', `/api/v1/catalog/productos/${producto.id}`);
  if (catalogo.json?.stock === 9) ok(`stock=9`);
  else fail(`stock=${catalogo.json?.stock}`);

  // ── 5. Logística: avance del ciclo de vida (Tabla 13) ─────────────────
  for (const [estado, descripcion] of [
    ['en_preparacion', 'la bodega alista el pedido'],
    ['enviada', 'guía de despacho generada'],
    ['entregada', 'el comprador confirma la recepción'],
  ]) {
    paso(`Admin avanza la orden → ${estado} (${descripcion})`);
    const avance = await api('PATCH', `/api/v1/orders/${orderId}/estado`, { estado }, tokenAdmin);
    if (avance.status === 200 && avance.json?.estado === estado) ok();
    else fail(`avance → ${avance.status} ${JSON.stringify(avance.json)}`);
  }

  // ── 6. Comisiones (TC-08, RN-04) ──────────────────────────────────────
  paso('TC-08: comisión 12 % acreditada (C$ 1 150,00 → C$ 138,00)');
  const estadoCuenta = await esperar(
    async () => {
      const r = await api('GET', '/api/v1/vendedores/me/ventas', null, tokenVendedor);
      return r.json?.ventas?.length >= 1 ? r.json : null;
    },
    'comisión acreditada',
  );
  const comision = estadoCuenta.ventas[0];
  if (comision.comision === '138.00' && comision.monto_vendedor === '1012.00') {
    ok(`comision=${comision.comision} liquida=${comision.monto_vendedor}`);
  } else {
    fail(`comision=${comision.comision} liquida=${comision.monto_vendedor}`);
  }

  // ── 7. TC-04: stock insuficiente → 409 (compra de 2 con stock total 1) ─
  paso('TC-04: producto con stock 1; compra de 2 unidades → 409');
  const prodUno = await api(
    'POST',
    '/api/v1/catalog/productos',
    { sku: `LEC-${suf.toUpperCase()}`, nombre: 'Leche 1L', precio_base: '50.00', stock: 1 },
    tokenAdmin,
  );
  const ofertaUno = await api(
    'POST',
    '/api/v1/vendedores/productos',
    { producto_id: prodUno.json.id, margen: 10 },
    tokenVendedor,
  );
  const rechazo = await api(
    'POST',
    '/api/v1/orders',
    { items: [{ oferta_id: ofertaUno.json.id, cantidad: 2 }] },
    tokenComprador,
  );
  if (rechazo.status === 409) ok();
  else fail(`esperaba 409, obtuve ${rechazo.status} ${JSON.stringify(rechazo.json)}`);

  // ── 8. Devolución (RN-06) ─────────────────────────────────────────────
  paso('Admin registra la devolución de la orden entregada (RN-06)');
  const devolucion = await api('PATCH', `/api/v1/orders/${orderId}/estado`, { estado: 'devuelta', motivo: 'cliente cambió de opinión' }, tokenAdmin);
  if (devolucion.status === 200) ok();
  else fail(`devolución → ${devolucion.status} ${JSON.stringify(devolucion.json)}`);

  paso('Stock reintegrado tras la devolución (RN-06)');
  const stockReintegrado = await esperar(
    async () => {
      const r = await api('GET', `/api/v1/catalog/productos/${producto.id}`);
      return r.json?.stock === 10 ? r.json : null;
    },
    'stock 10',
  );
  ok(`stock=${stockReintegrado.stock}`);

  paso('Compensación de comisión en la próxima liquidación (RN-06)');
  const compensada = await esperar(
    async () => {
      const r = await api('GET', '/api/v1/vendedores/me/ventas', null, tokenVendedor);
      return r.json?.ventas?.[0]?.estado === 'compensada' ? r.json : null;
    },
    'comisión compensada',
  );
  ok(`estado=${compensada.ventas[0].estado}`);

  // ── 9. Liquidación quincenal (RN-07) + segunda venta exitosa ──────────
  paso('Nueva venta que sí se entrega (para liquidación)');
  const orden2 = await api(
    'POST',
    '/api/v1/orders',
    { items: [{ oferta_id: oferta.json.id, cantidad: 2 }] },
    tokenComprador,
  );
  const orderId2 = orden2.json.id;
  await esperar(
    async () => {
      const r = await api('GET', `/api/v1/orders/${orderId2}`, null, tokenComprador);
      return r.json?.estado === 'pagada' ? r.json : null;
    },
    'orden2 pagada',
  );
  for (const estado of ['en_preparacion', 'enviada', 'entregada']) {
    await api('PATCH', `/api/v1/orders/${orderId2}/estado`, { estado }, tokenAdmin);
  }
  await esperar(
    async () => {
      const r = await api('GET', '/api/v1/vendedores/me/ventas', null, tokenVendedor);
      return r.json?.comisiones_pendientes === '276.00' ? r.json : null;
    },
    'comisión 2x1150=276.00',
  );
  ok('comisión pendiente de 276.00');

  paso('Corte de liquidación quincenal (RN-07, días 1 y 15)');
  const corte = await api('POST', '/api/v1/admin/liquidaciones/corte', null, tokenAdmin);
  if (corte.status === 200 && Array.isArray(corte.json)) {
    const conFondos = corte.json.find((l) => l.monto_cents > 0);
    if (conFondos) ok(`liquidación aprobada por C$${(conFondos.monto_cents / 100).toFixed(2)}`);
    else ok(`sin cortes con fondos (${corte.json.length} registros)`);
  } else {
    fail(`corte → ${corte.status} ${JSON.stringify(corte.json)}`);
  }

  // ── 10. Reportes admin (H-06) ─────────────────────────────────────────
  paso('Reportes del administrador (H-06: GMV, pedidos, inventario)');
  const reportes = await api('GET', '/api/v1/admin/reportes', null, tokenAdmin);
  if (reportes.status === 200 && reportes.json?.gmv_mes) {
    ok(`gmv_mes=${reportes.json.gmv_mes} pedidos=${reportes.json.pedidos_mes} stock_total=${reportes.json.stock_total}`);
  } else {
    fail(`reportes → ${reportes.status} ${JSON.stringify(reportes.json)}`);
  }

  paso('Inventario del administrador');
  const inventario = await api('GET', '/api/v1/admin/inventario', null, tokenAdmin);
  if (inventario.status === 200 && inventario.json.length >= 2) ok(`líneas=${inventario.json.length}`);
  else fail(`inventario → ${inventario.status}`);

  console.log('\n');
  console.log('═══════════════════════════════════════════════════');
  console.log(`Resultado de la demo: ${pasos} pasos, ${fallidas} fallos`);
  console.log('═══════════════════════════════════════════════════');
  if (fallidas > 0) process.exit(1);
}

main().catch((err) => {
  console.error('\nLa demo falló antes de completarse:', err.message);
  process.exit(1);
});