import http from 'k6/http';
import { check, sleep } from 'k6';

// Prueba de carga acotada. NO es una prueba de estrés ni un DoS.
//
// - Solo golpea /health (liveness barato, sin fan-out).
// - Techo duro: 20 VUs y 90 s de meseta.
// - Aborta si el error rate supera el 10 % o la p95 supera 1,5 s.
//
// Uso (siempre con aprobación y monitorización del VPS):
//   k6 run security/k6/health-load.js
//   BASE_URL=https://api.kbcoleccion.com k6 run security/k6/health-load.js
export const options = {
  scenarios: {
    load: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 10 },
        { duration: '30s', target: 20 },
        { duration: '60s', target: 20 },
        { duration: '15s', target: 0 },
      ],
      gracefulRampDown: '10s',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.10'],
    http_req_duration: ['p(95)<1500'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'https://api.kbcoleccion.com';

export default function () {
  const res = http.get(`${BASE_URL}/health`, {
    tags: { name: 'GET /health' },
  });

  check(res, {
    'status is 200': (r) => r.status === 200,
  });

  // ~2 req/s por VU como máximo.
  sleep(1.5);
}
