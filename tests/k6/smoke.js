import { check, sleep } from 'k6';
import { endpoints, handleSummary, login } from './lib.js';

/**
 * Smoke funcional bajo carga mínima: valida que las rutas críticas respondan
 * antes de lanzar una prueba mayor. 2 VUs, 20 s.
 */
export const options = {
  scenarios: {
    smoke: {
      executor: 'constant-vus',
      vus: 2,
      duration: '20s',
    },
  },
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(90)', 'p(95)', 'p(99)'],
  thresholds: {
    http_req_failed: ['rate<0.01'],
    http_req_duration: ['p(95)<1500'],
  },
};

export function setup() {
  return { token: login() };
}

export default function (data) {
  for (const item of endpoints(data.token)) {
    const res = item.run();
    check(res, { 'status 200': (r) => r.status === 200 });
  }
  sleep(0.5);
}

export { handleSummary };
