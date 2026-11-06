import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * Rate limiting en el perímetro (R-DS-6 / R-GW-2).
 *
 * El gateway debe mantener un cubo estricto para login y un cubo global, y
 * /ready no puede quedar sin límite.
 */

const MIDDLEWARE = readText(
  join(repoRoot, 'packages/api-gateway/src/proxy/rate-limit.middleware.ts'),
);
const MAIN = readText(join(repoRoot, 'packages/api-gateway/src/main.ts'));

export default defineSuite({
  id: 'security-rate-limit',
  area: 'security',
  rules: ['R-DS-6', 'R-GW-2'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-6: el login tiene un cubo estricto separado del global', () => {
      assert.ok(MIDDLEWARE.includes('this.login'), 'falta el cubo de login');
      assert.ok(MIDDLEWARE.includes('this.globales'), 'falta el cubo global');
      assert.ok(MIDDLEWARE.includes('esLogin'), 'no se distingue POST /auth/login');
      assert.ok(MIDDLEWARE.includes('/auth/login'), 'falta la ruta /auth/login');
    });

    test('R-DS-6: el limitador responde 429 con Retry-After', () => {
      assert.ok(MIDDLEWARE.includes("'x-ratelimit-limit'"), 'falta x-ratelimit-limit');
      assert.ok(MIDDLEWARE.includes("'Retry-After'"), 'falta Retry-After');
      assert.ok(MIDDLEWARE.includes('DEMASIADAS_PETICIONES'), 'falta el error 429');
    });

    test('R-DS-6: /ready pasa por el limitador', () => {
      assert.ok(MAIN.includes("expressApp.get('/ready'"), 'falta la ruta /ready');
      assert.ok(MAIN.includes('limitador.consumir'), '/ready no consume el limitador');
    });
  },
});
