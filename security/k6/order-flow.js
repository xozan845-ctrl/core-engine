import http from 'k6/http';
import { check, fail, sleep } from 'k6';

// Carga acotada del camino crítico: auth → empresa → producto → oferta →
// orden → pago. NO es una prueba de estrés ni un DoS.
//
// - 5 VUs máximo y 70 s de escenario.
// - Crea un tenant/producto efímeros con stock 5000 en `setup()`.
// - Por defecto apunta a localhost; para stage/producción exige aprobación y
//   monitorización (R-CD-12).
//
// Uso:
//   k6 run security/k6/order-flow.js
//   BASE_URL=https://api.staging.example k6 run security/k6/order-flow.js
//
// Variables: BASE_URL, ADMIN_EMAIL, ADMIN_PASSWORD (los mismos del smoke).
export const options = {
  scenarios: {
    orders: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '15s', target: 5 },
        { duration: '45s', target: 5 },
        { duration: '10s', target: 0 },
      ],
      gracefulRampDown: '10s',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.01'],
    'http_req_duration{name:POST /api/v2/orders}': ['p(95)<1500'],
    'http_req_duration{name:GET /api/v2/orders/:id}': ['p(95)<800'],
    checks: ['rate>0.99'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const ADMIN_EMAIL = __ENV.ADMIN_EMAIL || 'admin@core-engine.test';
const ADMIN_PASSWORD = __ENV.ADMIN_PASSWORD || 'AdminCore Engine2026!';
const PASSWORD = 'Contrasena123!';

function sufijo() {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

function desenvolver(res) {
  let body;
  try {
    body = res.json();
  } catch {
    return null;
  }
  if (
    body &&
    typeof body === 'object' &&
    !Array.isArray(body) &&
    Object.prototype.hasOwnProperty.call(body, 'data') &&
    body.meta &&
    typeof body.meta === 'object'
  ) {
    return body.data;
  }
  return body;
}

function api(metodo, ruta, cuerpo, token, nombre) {
  return http.request(metodo, `${BASE_URL}${ruta}`, cuerpo ? JSON.stringify(cuerpo) : null, {
    headers: {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    tags: { name: nombre ?? `${metodo} ${ruta.split('?')[0]}` },
  });
}

function crear(cuerpo, etiqueta) {
  if (!cuerpo) fail(`${etiqueta}: respuesta sin cuerpo`);
  return cuerpo;
}

export function setup() {
  const suf = sufijo();
  const correoEmpresario = `carga.emp.${suf}@core-engine.test`;
  const correoVendedor = `carga.vend.${suf}@core-engine.test`;
  const correoComprador = `carga.comp.${suf}@core-engine.test`;

  const admin = crear(
    desenvolver(api('POST', '/api/v2/auth/login', { correo: ADMIN_EMAIL, contrasena: ADMIN_PASSWORD })),
    'login admin',
  );
  if (!admin.access_token) fail('login admin: sin access_token');

  const empresario = crear(
    desenvolver(api('POST', '/api/v2/auth/registro', {
      nombre: 'Carga Empresario',
      correo: correoEmpresario,
      contrasena: PASSWORD,
      rol: 'empresario',
    })),
    'registro empresario',
  );
  const comprador = crear(
    desenvolver(api('POST', '/api/v2/auth/registro', {
      nombre: 'Carga Comprador',
      correo: correoComprador,
      contrasena: PASSWORD,
      rol: 'comprador',
    })),
    'registro comprador',
  );

  const empresa = crear(
    desenvolver(api('POST', '/api/v2/empresas', { nombre: `Carga ${suf}` }, empresario.access_token)),
    'crear empresa',
  );

  // La creación de la empresa asigna tenant_id; se renueva el token para RLS.
  const empresarioActivo = crear(
    desenvolver(api('POST', '/api/v2/auth/login', { correo: correoEmpresario, contrasena: PASSWORD })),
    'login empresario con tenant',
  );

  crear(
    desenvolver(api('POST', `/api/v2/empresas/${empresa.id}/vendedores`, {
      nombre: 'Carga Vendedor',
      correo: correoVendedor,
      contrasena: PASSWORD,
      participacion_porcentaje: 100,
    }, empresarioActivo.access_token)),
    'crear vendedor',
  );

  const vendedor = crear(
    desenvolver(api('POST', '/api/v2/auth/login', { correo: correoVendedor, contrasena: PASSWORD })),
    'login vendedor',
  );

  crear(
    desenvolver(api('POST', '/api/v2/vendedores/tienda', { nombre: `Tienda Carga ${suf}` }, vendedor.access_token)),
    'crear tienda',
  );

  const producto = crear(
    desenvolver(api('POST', '/api/v2/catalog/items', {
      tipo: 'producto',
      codigo: `CARGA-${suf.toUpperCase()}`,
      empresa_id: empresa.id,
      nombre: 'Producto de carga',
      descripcion: 'Producto efímero para la prueba k6',
      categoria: 'general',
      precio: '1000.00',
      stock: 5000,
    }, admin.access_token)),
    'crear producto',
  );

  const oferta = crear(
    desenvolver(api('POST', '/api/v2/vendedores/productos', {
      item_id: producto.item_id ?? producto.id,
      margen: 15,
    }, vendedor.access_token)),
    'crear oferta',
  );

  return { ofertaId: oferta.id, tokenComprador: comprador.access_token };
}

export default function (data) {
  const creada = api(
    'POST',
    '/api/v2/orders',
    { tipo_orden: 'compra', items: [{ oferta_id: data.ofertaId, cantidad: 1 }] },
    data.tokenComprador,
    'POST /api/v2/orders',
  );
  const orden = desenvolver(creada);
  check(creada, {
    'orden creada': (r) => (r.status === 200 || r.status === 201) && Boolean(orden?.id),
  });
  if (!orden?.id) {
    sleep(0.5);
    return;
  }

  let estado;
  for (let intento = 0; intento < 12; intento += 1) {
    const consulta = api(
      'GET',
      `/api/v2/orders/${orden.id}`,
      null,
      data.tokenComprador,
      'GET /api/v2/orders/:id',
    );
    estado = desenvolver(consulta)?.estado;
    if (estado === 'pagada') break;
    sleep(0.5);
  }
  check({ estado }, {
    'orden pagada': (valor) => valor.estado === 'pagada',
  });
  sleep(0.5);
}
