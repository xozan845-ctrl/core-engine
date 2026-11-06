import { check } from 'k6';
import { endpoints, handleSummary, login, pick } from './lib.js';

/**
 * Breakpoint: sube el ritmo hasta encontrar el codo de capacidad.
 *
 * Aborta si el error rate supera 5 % o la p95 supera 2 s. Es una prueba
 * manual: no se ejecuta en el deploy automático ni contra producción sin
 * aprobación.
 *
 *   K6_START_RATE=10 K6_RATE_1=50 ... K6_RATE_5=800 \
 *   K6_STAGE_DURATION=45s k6 run tests/k6/breakpoint.js
 */
export const options = {
  scenarios: {
    breakpoint: {
      executor: 'ramping-arrival-rate',
      startRate: Number(__ENV.K6_BREAK_START_RATE || 10),
      timeUnit: '1s',
      preAllocatedVUs: Number(__ENV.K6_BREAK_PRE_VUS || 100),
      maxVUs: Number(__ENV.K6_BREAK_MAX_VUS || 500),
      stages: [
        { target: Number(__ENV.K6_BREAK_RATE_1 || 50), duration: __ENV.K6_BREAK_STAGE_DURATION || '45s' },
        { target: Number(__ENV.K6_BREAK_RATE_2 || 100), duration: __ENV.K6_BREAK_STAGE_DURATION || '45s' },
        { target: Number(__ENV.K6_BREAK_RATE_3 || 200), duration: __ENV.K6_BREAK_STAGE_DURATION || '45s' },
        { target: Number(__ENV.K6_BREAK_RATE_4 || 400), duration: __ENV.K6_BREAK_STAGE_DURATION || '45s' },
        { target: Number(__ENV.K6_BREAK_RATE_5 || 800), duration: __ENV.K6_BREAK_STAGE_DURATION || '45s' },
      ],
    },
  },
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(90)', 'p(95)', 'p(99)'],
  thresholds: {
    http_req_failed: [{ threshold: 'rate<0.05', abortOnFail: true, delayAbortEval: '10s' }],
    http_req_duration: [{ threshold: 'p(95)<2000', abortOnFail: true, delayAbortEval: '10s' }],
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
