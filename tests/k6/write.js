import http from 'k6/http';
import { check } from 'k6';
import { vu } from 'k6/execution';
import { BASE_URL, STORE_SLUG, handleSummary, headers } from './lib.js';

/**
 * Escritura: carrito + creación de orden (checkout).
 *
 * Usa un pool de compradores (uno por VU) para que cada carrito sea privado y
 * no haya carreras. Crea usuarios y órdenes reales: úsalo contra un tenant de
 * prueba, nunca contra producción sin ventana y limpieza.
 *
 *   K6_WRITE_RATE=5 K6_WRITE_DURATION=30s k6 run tests/k6/write.js
 */
const COMPRADORES = Number(__ENV.K6_WRITE_BUYERS || 20);
// Obligatoria: no se versiona una contraseña de fixture.
const PASSWORD = __ENV.K6_WRITE_PASSWORD || '';

export const options = {
  scenarios: {
    write: {
      executor: 'constant-arrival-rate',
      rate: Number(__ENV.K6_WRITE_RATE || 5),
      timeUnit: '1s',
      duration: __ENV.K6_WRITE_DURATION || '1m',
      preAllocatedVUs: Number(__ENV.K6_WRITE_PRE_VUS || 20),
      maxVUs: Number(__ENV.K6_WRITE_MAX_VUS || 100),
    },
  },
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(90)', 'p(95)', 'p(99)'],
  thresholds: {
    http_req_failed: ['rate<0.01'],
    checks: ['rate>0.98'],
    'http_req_duration{name:POST /cart}': ['p(95)<800'],
    'http_req_duration{name:POST /orders}': ['p(95)<1500'],
  },
};

function registrarComprador(indice) {
  if (!PASSWORD) {
    console.warn('K6_WRITE_PASSWORD no está definida; no se pueden crear compradores.');
    return null;
  }
  const correo = `k6.write.${Date.now()}.${indice}@example.test`;
  const res = http.post(
    `${BASE_URL}/api/v1/auth/registro`,
    JSON.stringify({ nombre: `K6 Buyer ${indice}`, correo, contrasena: PASSWORD, rol: 'comprador' }),
    {
      headers: headers(null, { 'Content-Type': 'application/json' }),
      tags: { name: 'POST /auth/registro (setup)' },
    },
  );
  if (res.status !== 201) {
    console.warn(`registro comprador ${indice}: ${res.status}`);
    return null;
  }
  try {
    return JSON.parse(res.body).access_token;
  } catch {
    return null;
  }
}

export function setup() {
  const tokens = [];
  for (let i = 0; i < COMPRADORES; i += 1) {
    const token = registrarComprador(i);
    if (token) tokens.push(token);
  }

  let ofertas = [];
  if (STORE_SLUG) {
    const res = http.get(`${BASE_URL}/api/v1/tiendas/${encodeURIComponent(STORE_SLUG)}`, {
      headers: headers(),
      tags: { name: 'GET /store (setup)' },
    });
    if (res.status === 200) {
      try {
        ofertas = (JSON.parse(res.body).ofertas ?? []).map((oferta) => oferta.id).filter(Boolean);
      } catch {
        ofertas = [];
      }
    }
  }
  return { tokens, ofertas };
}

export default function (data) {
  if (data.tokens.length === 0 || data.ofertas.length === 0) {
    check(null, { 'setup con compradores y ofertas': () => false });
    return;
  }

  const token = data.tokens[(vu.idInTest - 1) % data.tokens.length];
  const oferta = data.ofertas[Math.floor(Math.random() * data.ofertas.length)];

  const carrito = http.post(
    `${BASE_URL}/api/v1/carrito/items`,
    JSON.stringify({ oferta_id: oferta, cantidad: 1 }),
    {
      headers: headers(token, { 'Content-Type': 'application/json' }),
      tags: { name: 'POST /cart' },
    },
  );
  check(carrito, { 'carrito 2xx': (r) => r.status >= 200 && r.status < 300 });
  if (carrito.status < 200 || carrito.status >= 300) return;

  const orden = http.post(
    `${BASE_URL}/api/v1/orders`,
    JSON.stringify({ usar_carrito: true }),
    {
      headers: headers(token, { 'Content-Type': 'application/json' }),
      tags: { name: 'POST /orders' },
    },
  );
  check(orden, { 'orden 201': (r) => r.status === 201 });
}

export { handleSummary };
