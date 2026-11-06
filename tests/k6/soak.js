import http from 'k6/http';
import { check } from 'k6';
import { vu } from 'k6/execution';
import { BASE_URL, STORE_SLUG, endpoints, handleSummary, headers, login, pick } from './lib.js';

/**
 * Soak: ritmo moderado durante 30-60 min para detectar degradación, fugas de
 * memoria/DB o crecimiento de latencia.
 *
 * Por defecto es lectura + 2 % de checkout. El checkout crea órdenes reales y
 * consume stock; súbelo solo contra un tenant de prueba. `K6_SOAK_WRITE_PCT=0`
 * lo deja read-only.
 *
 *   K6_SOAK_RATE=20 K6_SOAK_DURATION=30m K6_SOAK_WRITE_PCT=2 k6 run tests/k6/soak.js
 */
const RATE = Number(__ENV.K6_SOAK_RATE || 50);
const WRITE_PCT = Math.min(100, Math.max(0, Number(__ENV.K6_SOAK_WRITE_PCT || 2))) / 100;
const DURATION = __ENV.K6_SOAK_DURATION || '30m';
const WRITE_RATE = Math.max(0, Math.round(RATE * WRITE_PCT));
const READ_RATE = Math.max(1, RATE - WRITE_RATE);
const WRITE_BUYERS = Number(__ENV.K6_SOAK_WRITE_BUYERS || 10);
const WRITE_PASSWORD = __ENV.K6_SOAK_WRITE_PASSWORD || __ENV.K6_WRITE_PASSWORD || '';

const scenarios = {
  reads: {
    executor: 'constant-arrival-rate',
    exec: 'leer',
    rate: READ_RATE,
    timeUnit: '1s',
    duration: DURATION,
    preAllocatedVUs: Number(__ENV.K6_SOAK_READ_VUS || 50),
    maxVUs: Number(__ENV.K6_SOAK_MAX_VUS || 200),
  },
};
if (WRITE_RATE > 0) {
  scenarios.writes = {
    executor: 'constant-arrival-rate',
    exec: 'escribir',
    rate: WRITE_RATE,
    timeUnit: '1s',
    duration: DURATION,
    preAllocatedVUs: Number(__ENV.K6_SOAK_WRITE_VUS || 10),
    maxVUs: Number(__ENV.K6_SOAK_WRITE_MAX_VUS || 50),
  };
}

export const options = {
  scenarios,
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(90)', 'p(95)', 'p(99)'],
  thresholds: {
    http_req_failed: ['rate<0.01'],
    http_req_duration: ['p(95)<800', 'p(99)<1500'],
    checks: ['rate>0.98'],
  },
};

function registrarComprador(indice) {
  if (!WRITE_PASSWORD) return null;
  const correo = `k6.soak.${Date.now()}.${indice}@example.test`;
  const res = http.post(
    `${BASE_URL}/api/v1/auth/registro`,
    JSON.stringify({ nombre: `K6 Soak ${indice}`, correo, contrasena: WRITE_PASSWORD, rol: 'comprador' }),
    {
      headers: headers(null, { 'Content-Type': 'application/json' }),
      tags: { name: 'POST /auth/registro (setup)' },
    },
  );
  if (res.status !== 201) return null;
  try {
    return JSON.parse(res.body).access_token;
  } catch {
    return null;
  }
}

export function setup() {
  const readToken = login();
  const writeTokens = [];
  let ofertas = [];

  if (WRITE_RATE > 0) {
    for (let i = 0; i < WRITE_BUYERS; i += 1) {
      const token = registrarComprador(i);
      if (token) writeTokens.push(token);
    }
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
  }

  return { readToken, writeTokens, ofertas };
}

export function leer(data) {
  const item = pick(endpoints(data.readToken));
  const res = item.run();
  check(res, { 'lectura < 500': (r) => r.status < 500 });
}

export function escribir(data) {
  if (data.writeTokens.length === 0 || data.ofertas.length === 0) {
    check(null, { 'setup de escritura': () => false });
    return;
  }
  const token = data.writeTokens[(vu.idInTest - 1) % data.writeTokens.length];
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
