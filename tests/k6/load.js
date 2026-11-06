import { check } from 'k6';
import { endpoints, handleSummary, login, pick } from './lib.js';

/**
 * Carga sostenida para el gate de no-regresión.
 *
 * El ritmo (`K6_RATE`) debe ser el 50-70 % de la capacidad medida con
 * `breakpoint.js`; así el gate detecta degradaciones sin tumbar el entorno.
 *
 *   K6_RATE=20 K6_DURATION=2m BASE_URL=http://127.0.0.1:8080 k6 run tests/k6/load.js
 */
export const options = {
  scenarios: {
    mix: {
      executor: 'constant-arrival-rate',
      rate: Number(__ENV.K6_LOAD_RATE || 20),
      timeUnit: '1s',
      duration: __ENV.K6_LOAD_DURATION || '2m',
      preAllocatedVUs: Number(__ENV.K6_LOAD_PRE_VUS || 50),
      maxVUs: Number(__ENV.K6_LOAD_MAX_VUS || 300),
    },
  },
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(90)', 'p(95)', 'p(99)'],
  thresholds: {
    http_req_failed: ['rate<0.01'],
    http_req_duration: ['p(95)<800', 'p(99)<1500'],
    'http_req_duration{name:GET /catalog}': ['p(95)<600'],
    'http_req_duration{name:GET /store}': ['p(95)<800'],
    'http_req_duration{name:GET /orders}': ['p(95)<800'],
  },
};

export function setup() {
  return { token: login() };
}

export default function (data) {
  const item = pick(endpoints(data.token));
  const res = item.run();
  check(res, { 'status < 500': (r) => r.status < 500 });
}

export { handleSummary };
