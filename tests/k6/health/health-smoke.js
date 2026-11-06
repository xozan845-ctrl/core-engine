import http from 'k6/http';
import { check, sleep } from 'k6';

// Baseline seguro: 5 VUs durante 20 s contra el liveness del gateway.
// /health es barato y no consulta microservicios.
export const options = {
  scenarios: {
    smoke: {
      executor: 'constant-vus',
      vus: 5,
      duration: '20s',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.05'],
    http_req_duration: ['p(95)<800'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'https://api.kbcoleccion.com';

export default function () {
  const res = http.get(`${BASE_URL}/health`, {
    tags: { name: 'GET /health' },
  });

  check(res, {
    'status is 200': (r) => r.status === 200,
    'body reports ok': (r) => typeof r.body === 'string' && r.body.includes('"ok"'),
  });

  sleep(2);
}
