/**
 * Core Engine · Core Engine
 * Demo del flujo real (entregable cap. 7.1): alta de empresa y vendedor
 * (ADR-25), producto, oferta, compra, descuento de stock, orden, comisión y
 * liquidación.
 *
 * Requiere el ecosistema levantado: docker compose -f docker-compose.dev.yml up -d --build
 * Uso: npm run demo  (o `node scripts/cli.mjs smoke`)
 */

import { corteLiquidacionesValido } from './smoke-contract.mjs';

const BASE = process.env.BASE_URL ?? 'http://localhost:8080';
const ADMIN_EMAIL = process.env.ADMIN_EMAIL ?? 'admin@core-engine.test';
const ADMIN_PASSWORD = process.env.ADMIN_PASSWORD ?? 'AdminCore Engine2026!';
const REQUEST_TIMEOUT_MS = Number(process.env.SMOKE_REQUEST_TIMEOUT_MS ?? 10_000);
const WAIT_TIMEOUT_MS = Number(process.env.SMOKE_WAIT_TIMEOUT_MS ?? 30_000);

for (const [nombre, valor] of [
  ['SMOKE_REQUEST_TIMEOUT_MS', REQUEST_TIMEOUT_MS],
  ['SMOKE_WAIT_TIMEOUT_MS', WAIT_TIMEOUT_MS],
]) {
  if (!Number.isInteger(valor) || valor < 1) {
    throw new Error(`${nombre} debe ser un entero positivo`);
  }
}

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

async function api(metodo, ruta, cuerpo, token, opciones = {}) {
  const intentos = metodo === 'GET' ? 2 : 1;
  for (let intento = 0; intento < intentos; intento += 1) {
    try {
      const res = await fetch(`${BASE}${ruta}`, {
        method: metodo,
        headers: {
          'content-type': 'application/json',
          ...(token ? { authorization: `Bearer ${token}` } : {}),
        },
        body: cuerpo ? JSON.stringify(cuerpo) : undefined,
        redirect: opciones.redirect ?? 'follow',
        signal: globalThis.AbortSignal.timeout(REQUEST_TIMEOUT_MS),
      });
      const texto = await res.text();
      let json;
      try {
        json = texto ? JSON.parse(texto) : null;
      } catch {
        json = texto;
      }
      return { status: res.status, json, location: res.headers.get('location') ?? undefined };
    } catch (error) {
      if (intento === intentos - 1) {
        throw new Error(`${metodo} ${ruta}: sin respuesta tras ${intentos} intento(s) de ${REQUEST_TIMEOUT_MS} ms`, { cause: error });
      }
    }
  }
}

async function esperar(fn, descripcion, intentos = 20, intervaloMs = 800) {
  let ultimo = null;
  const deadline = Date.now() + WAIT_TIMEOUT_MS;
  for (let i = 0; i < intentos; i += 1) {
    if (Date.now() >= deadline) break;
    ultimo = await fn();
    if (ultimo) return ultimo;
    const restante = deadline - Date.now();
    if (restante > 0) {
      await new Promise((r) => setTimeout(r, Math.min(intervaloMs, restante)));
    }
  }
  throw new Error(`Expiró la espera de ${WAIT_TIMEOUT_MS} ms: ${descripcion} (último: ${JSON.stringify(ultimo)})`);
}

function sufijo() {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

async function main() {
  paso('Readiness del ecosistema (R-CD-12: todos los servicios en pie)');
  const ready = await api('GET', '/ready');
  if (ready.status === 200 && ready.json?.status === 'ok') {
    ok(`servicios ok: ${Object.keys(ready.json.servicios ?? {}).join(', ')}`);
  } else {
    fail(`readiness=${ready.status} ${JSON.stringify(ready.json?.servicios ?? ready.json)}`);
    process.exit(1);
  }

  // ── 1. Identidad ──────────────────────────────────────────────────────
  const suf = sufijo();
  const correoEmpresario = `empresario.${suf}@core-engine.test`;
  const correoVendedor = `vendedor.${suf}@core-engine.test`;
  const correoComprador = `comprador.${suf}@core-engine.test`;
  const contrasena = 'Contrasena123!';

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

  paso('Registro de empresario y comprador (RF-01)');
  const [regE, regC] = await Promise.all([
    api('POST', '/api/v1/auth/registro', {
      nombre: 'Empresaria Piloto',
      correo: correoEmpresario,
      contrasena,
      rol: 'empresario',
    }),
    api('POST', '/api/v1/auth/registro', {
      nombre: 'Comprador Piloto',
      correo: correoComprador,
      contrasena,
      rol: 'comprador',
    }),
  ]);
  if (![200, 201].includes(regE.status) || !regE.json?.access_token) {
    fail(`registro empresario → ${regE.status} ${JSON.stringify(regE.json)}`);
    process.exit(1);
  }
  if (![200, 201].includes(regC.status) || !regC.json?.access_token) {
    fail(`registro comprador → ${regC.status} ${JSON.stringify(regC.json)}`);
    process.exit(1);
  }
  ok('empresario y comprador registrados');
  const tokenEmpresario = regE.json.access_token;
  const tokenComprador = regC.json.access_token;

  // ── 1b. Empresa y vendedor (ADR-25) ──────────────────────────────────
  paso('El empresario crea su empresa (ADR-25)');
  const empresa = await api(
    'POST',
    '/api/v1/empresas',
    { nombre: 'Distribuidora Piloto', modalidad_logistica: 'plataforma' },
    tokenEmpresario,
  );
  if (![200, 201].includes(empresa.status) || !empresa.json?.id) {
    fail(`crear empresa → ${empresa.status} ${JSON.stringify(empresa.json)}`);
    process.exit(1);
  }
  ok(`empresa=${empresa.json.id.slice(0, 8)}`);

  paso('El empresario crea al vendedor con participación 100 % (ADR-25)');
  const vendedor = await api(
    'POST',
    `/api/v1/empresas/${empresa.json.id}/vendedores`,
    {
      nombre: 'Vendedora Piloto',
      correo: correoVendedor,
      contrasena,
      participacion_porcentaje: 100,
    },
    tokenEmpresario,
  );
  if (![200, 201].includes(vendedor.status) || !vendedor.json?.id) {
    fail(`crear vendedor → ${vendedor.status} ${JSON.stringify(vendedor.json)}`);
    process.exit(1);
  }
  ok(`vendedor=${vendedor.json.id.slice(0, 8)}`);

  paso('Vendedor: login con la cuenta creada por su empresa');
  const loginVendedor = await api('POST', '/api/v1/auth/login', {
    correo: correoVendedor,
    contrasena,
  });
  if (loginVendedor.status === 200 && loginVendedor.json?.access_token) {
    ok();
  } else {
    fail(`login vendedor → ${loginVendedor.status} ${JSON.stringify(loginVendedor.json)}`);
    process.exit(1);
  }
  const tokenVendedor = loginVendedor.json.access_token;

  // ── 1c. Slugs de empresa (ADR-26) ────────────────────────────────────
  paso('La empresa se resuelve por UUID y por slug (ADR-26)');
  const empresaPorId = await api('GET', `/api/v1/empresas/${empresa.json.id}`, null, tokenEmpresario);
  const empresaPorSlug = await api('GET', `/api/v1/empresas/${empresa.json.slug}`, null, tokenEmpresario);
  if (
    empresa.json.slug &&
    empresaPorId.status === 200 &&
    empresaPorSlug.status === 200 &&
    empresaPorSlug.json?.id === empresa.json.id
  ) {
    ok(`slug=${empresa.json.slug}`);
  } else {
    fail(`resolución por UUID/slug → ${empresaPorId.status}/${empresaPorSlug.status}`);
  }

  paso('El slug de empresa no es público: exige sesión y rol autorizado (ADR-26)');
  const empresaSinSesion = await api('GET', `/api/v1/empresas/${empresa.json.slug}`);
  const empresaConComprador = await api(
    'GET',
    `/api/v1/empresas/${empresa.json.slug}`,
    null,
    tokenComprador,
  );
  if (empresaSinSesion.status === 401 && empresaConComprador.status === 403) {
    ok('401 sin token, 403 para comprador');
  } else {
    fail(`esperaba 401/403, obtuve ${empresaSinSesion.status}/${empresaConComprador.status}`);
  }

  paso('El slug anterior de la empresa redirige 301 al actual (ADR-26)');
  const slugEmpresaNuevo = `${empresa.json.slug}-nueva`;
  const cambioSlugEmpresa = await api(
    'PATCH',
    `/api/v1/empresas/${empresa.json.id}/slug`,
    { slug: slugEmpresaNuevo },
    tokenEmpresario,
  );
  const empresaSlugAnterior = await api(
    'GET',
    `/api/v1/empresas/${empresa.json.slug}`,
    null,
    tokenEmpresario,
    { redirect: 'manual' },
  );
  const empresaSlugNuevo = await api(
    'GET',
    `/api/v1/empresas/${slugEmpresaNuevo}`,
    null,
    tokenEmpresario,
  );
  if (
    cambioSlugEmpresa.status === 200 &&
    cambioSlugEmpresa.json?.slug === slugEmpresaNuevo &&
    empresaSlugAnterior.status === 301 &&
    empresaSlugAnterior.location === `/api/v1/empresas/${slugEmpresaNuevo}` &&
    empresaSlugNuevo.status === 200 &&
    empresaSlugNuevo.json?.id === empresa.json.id
  ) {
    ok(`301 ${empresa.json.slug} → ${slugEmpresaNuevo}`);
  } else {
    fail(
      `cambio=${cambioSlugEmpresa.status} anterior=${empresaSlugAnterior.status} ` +
        `location=${empresaSlugAnterior.location} nuevo=${empresaSlugNuevo.status}`,
    );
  }

  // ── 2. Catálogo (TC-01) ───────────────────────────────────────────────
  paso('Admin da de alta el producto (TC-01: stock 10, precio base C$ 1 000)');
  const crear = await api(
    'POST',
    '/api/v1/catalog/productos',
    { sku: `ACE-${suf.toUpperCase()}`, empresa_id: empresa.json.id, nombre: 'Aceite de cocina 1L', descripcion: 'Aceite vegetal', categoria: 'abarrotes', precio_base: '1000.00', stock: 10 },
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

  paso('La tienda se resuelve por slug en abierto (ADR-26)');
  const tiendaPorSlug = await api('GET', `/api/v1/tiendas/${tienda.json.slug}`);
  if (
    tienda.json.slug &&
    tiendaPorSlug.status === 200 &&
    tiendaPorSlug.json?.tienda?.id === tienda.json.id
  ) {
    ok(`slug=${tienda.json.slug}`);
  } else {
    fail(`tienda por slug → ${tiendaPorSlug.status} ${JSON.stringify(tiendaPorSlug.json)}`);
  }

  paso('El slug anterior de la tienda redirige 301 al actual (ADR-26)');
  const slugTiendaNuevo = `${tienda.json.slug}-nueva`;
  const cambioSlugTienda = await api(
    'PATCH',
    `/api/v1/tiendas/${tienda.json.id}/slug`,
    { slug: slugTiendaNuevo },
    tokenVendedor,
  );
  const tiendaSlugAnterior = await api(
    'GET',
    `/api/v1/tiendas/${tienda.json.slug}`,
    null,
    null,
    { redirect: 'manual' },
  );
  const tiendaSlugNuevo = await api('GET', `/api/v1/tiendas/${slugTiendaNuevo}`);
  if (
    cambioSlugTienda.status === 200 &&
    cambioSlugTienda.json?.slug === slugTiendaNuevo &&
    tiendaSlugAnterior.status === 301 &&
    tiendaSlugAnterior.location === `/api/v1/tiendas/${slugTiendaNuevo}` &&
    tiendaSlugNuevo.status === 200 &&
    tiendaSlugNuevo.json?.tienda?.id === tienda.json.id
  ) {
    ok(`301 ${tienda.json.slug} → ${slugTiendaNuevo}`);
  } else {
    fail(
      `cambio=${cambioSlugTienda.status} anterior=${tiendaSlugAnterior.status} ` +
        `location=${tiendaSlugAnterior.location} nuevo=${tiendaSlugNuevo.status}`,
    );
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
    { sku: `LEC-${suf.toUpperCase()}`, empresa_id: empresa.json.id, nombre: 'Leche 1L', precio_base: '50.00', stock: 1 },
    tokenAdmin,
  );
  if (![200, 201].includes(prodUno.status) || !prodUno.json?.id) {
    fail(`producto con stock 1 → ${prodUno.status} ${JSON.stringify(prodUno.json)}`);
    process.exit(1);
  }
  const ofertaUno = await api(
    'POST',
    '/api/v1/vendedores/productos',
    { producto_id: prodUno.json.id, margen: 10 },
    tokenVendedor,
  );
  if (![200, 201].includes(ofertaUno.status) || !ofertaUno.json?.id) {
    fail(`oferta con stock 1 → ${ofertaUno.status} ${JSON.stringify(ofertaUno.json)}`);
    process.exit(1);
  }
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
  if (![200, 201].includes(orden2.status) || !orden2.json?.id) {
    fail(`segunda orden → ${orden2.status} ${JSON.stringify(orden2.json)}`);
    process.exit(1);
  }
  const orderId2 = orden2.json.id;
  await esperar(
    async () => {
      const r = await api('GET', `/api/v1/orders/${orderId2}`, null, tokenComprador);
      return r.json?.estado === 'pagada' ? r.json : null;
    },
    'orden2 pagada',
  );
  for (const estado of ['en_preparacion', 'enviada', 'entregada']) {
    const avance = await api('PATCH', `/api/v1/orders/${orderId2}/estado`, { estado }, tokenAdmin);
    if (avance.status !== 200 || avance.json?.estado !== estado) {
      fail(`segunda orden, avance ${estado} → ${avance.status} ${JSON.stringify(avance.json)}`);
      process.exit(1);
    }
  }
  await esperar(
    async () => {
      const r = await api('GET', '/api/v1/vendedores/me/ventas', null, tokenVendedor);
      return r.json?.comisiones_pendientes === '2024.00' ? r.json : null;
    },
    'neto pendiente 2x1150 - 12% = 2024.00',
  );
  ok('comisión pendiente de 2024.00');

  paso('Corte de liquidación quincenal (RN-07, días 1 y 15)');
  const corte = await api('POST', '/api/v1/admin/liquidaciones/corte', null, tokenAdmin);
  if (corteLiquidacionesValido(corte)) {
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
