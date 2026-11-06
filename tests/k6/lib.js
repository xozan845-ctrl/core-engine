import http from 'k6/http';
import encoding from 'k6/encoding';

/**
 * Utilidades compartidas por los escenarios k6.
 *
 * Variables:
 *   BASE_URL             URL del gateway (default http://127.0.0.1:8080)
 *   K6_EMPRESA_ID        empresa del catálogo público (sin ella se omite)
 *   K6_STORE_SLUG        slug de la tienda pública (sin él se omite)
 *   K6_BUYER_EMAIL/PASS  cuenta compradora para /orders (opcional)
 *   K6_READY=true        incluye /ready (necesita K6_INTERNAL_KEY en stage/prod)
 *   K6_INTERNAL_KEY      x-internal-key para /ready y diagnóstico
 *   K6_SUMMARY           ruta del summary JSON (default k6-summary.json)
 */

export const BASE_URL = __ENV.BASE_URL || 'http://127.0.0.1:8080';
export const EMPRESA_ID = __ENV.K6_EMPRESA_ID || '';
export const STORE_SLUG = __ENV.K6_STORE_SLUG || '';
export const BUYER_EMAIL = __ENV.K6_BUYER_EMAIL || '';
export const BUYER_PASSWORD = __ENV.K6_BUYER_PASSWORD || '';
export const READY = String(__ENV.K6_READY || 'false').toLowerCase() === 'true';
export const INTERNAL_KEY = __ENV.K6_INTERNAL_KEY || '';
export const EDGE_BASIC_AUTH = __ENV.K6_EDGE_BASIC_AUTH || '';

/** XFF por VU/iteración para no compartir el cubo de rate-limit entre VUs. */
export function headers(token, extra = {}) {
  // XFF aleatorio: simula muchos clientes y evita que el rate-limit por IP
  // (300/min) enmascare la capacidad real del backend. El login de setup usa
  // una IP fija y solo se ejecuta una vez.
  const a = 1 + Math.floor(Math.random() * 250);
  const b = 1 + Math.floor(Math.random() * 250);
  const h = {
    'x-forwarded-for': `10.201.${a}.${b}`,
    ...extra,
  };
  if (token) h.Authorization = `Bearer ${token}`;
  else if (EDGE_BASIC_AUTH) h.Authorization = `Basic ${encoding.b64encode(EDGE_BASIC_AUTH)}`;
  if (INTERNAL_KEY) h['x-internal-key'] = INTERNAL_KEY;
  return h;
}

/** Login único de setup; si falla, los escenarios autenticados se omiten. */
export function login() {
  if (!BUYER_EMAIL || !BUYER_PASSWORD) return null;
  const res = http.post(
    `${BASE_URL}/api/v1/auth/login`,
    JSON.stringify({ correo: BUYER_EMAIL, contrasena: BUYER_PASSWORD }),
    {
      headers: headers(null, { 'Content-Type': 'application/json' }),
      tags: { name: 'POST /auth/login (setup)' },
    },
  );
  if (res.status !== 200) return null;
  try {
    return JSON.parse(res.body).access_token;
  } catch {
    return null;
  }
}

/** Selección ponderada simple. */
export function pick(items) {
  const total = items.reduce((suma, item) => suma + item.weight, 0);
  let punto = Math.random() * total;
  for (const item of items) {
    punto -= item.weight;
    if (punto <= 0) return item;
  }
  return items[items.length - 1];
}

/** Mezcla de endpoints: catálogo, tienda, liveness, pedidos y readiness. */
export function endpoints(token) {
  const items = [];
  if (EMPRESA_ID) {
    items.push({
      weight: 45,
      run: () =>
        http.get(
          `${BASE_URL}/api/v1/catalog/productos?empresa_id=${EMPRESA_ID}&limit=20`,
          { headers: headers(), tags: { name: 'GET /catalog' } },
        ),
    });
  }
  if (STORE_SLUG) {
    items.push({
      weight: 30,
      run: () =>
        http.get(`${BASE_URL}/api/v1/tiendas/${encodeURIComponent(STORE_SLUG)}`, {
          headers: headers(),
          tags: { name: 'GET /store' },
        }),
    });
  }
  items.push({
    weight: 10,
    run: () => http.get(`${BASE_URL}/health`, { headers: headers(), tags: { name: 'GET /health' } }),
  });
  if (READY) {
    items.push({
      weight: 5,
      run: () => http.get(`${BASE_URL}/ready`, { headers: headers(), tags: { name: 'GET /ready' } }),
    });
  }
  if (token) {
    items.push({
      weight: 10,
      run: () =>
        http.get(`${BASE_URL}/api/v1/orders`, {
          headers: headers(token),
          tags: { name: 'GET /orders' },
        }),
    });
  }
  return items;
}

/** Resumen JSON para el comparador de línea base. */
export function handleSummary(data) {
  const salida = __ENV.K6_SUMMARY || 'k6-summary.json';
  const metricas = data.metrics;
  const valor = (nombre, clave) => metricas[nombre]?.values?.[clave];
  const lineas = [
    `http_reqs:        ${(valor('http_reqs', 'rate') ?? 0).toFixed(2)} req/s`,
    `http_req_failed:  ${((valor('http_req_failed', 'rate') ?? 0) * 100).toFixed(2)} %`,
    `http_req_duration p95: ${(valor('http_req_duration', 'p(95)') ?? 0).toFixed(0)} ms`,
    `http_req_duration p99: ${(valor('http_req_duration', 'p(99)') ?? 0).toFixed(0)} ms`,
  ];
  return {
    [salida]: JSON.stringify(data, null, 2),
    stdout: `${lineas.join('\n')}\n`,
  };
}
