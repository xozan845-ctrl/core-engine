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

const LIMITE = readText(join(repoRoot, 'packages/api-gateway/src/rate_limit.rs'));
const HTTP = readText(join(repoRoot, 'packages/api-gateway/src/http/mod.rs'));

export default defineSuite({
  id: 'security-rate-limit',
  area: 'security',
  rules: ['R-DS-6', 'R-GW-2'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-6: el login tiene un cubo estricto separado del global', () => {
      assert.ok(LIMITE.includes('es_login'), 'no se distingue POST /auth/login');
      assert.ok(LIMITE.includes('/auth/login'), 'falta la ruta /auth/login');
      assert.ok(HTTP.includes('"login"'), 'falta el cubo de login');
      assert.ok(HTTP.includes('"global"'), 'falta el cubo global');
    });

    test('R-DS-6: el limitador responde 429 con Retry-After', () => {
      assert.ok(HTTP.includes('"x-ratelimit-limit"'), 'falta x-ratelimit-limit');
      assert.ok(HTTP.includes('RETRY_AFTER'), 'falta Retry-After');
      assert.ok(HTTP.includes('DEMASIADAS_PETICIONES'), 'falta el error 429');
    });

    test('R-DS-6: /ready pasa por el limitador', () => {
      assert.ok(HTTP.includes('ruta == "/ready"'), 'falta la ruta /ready en el limitador');
      assert.ok(
        /\.rate\s*\.\s*consumir\(/.test(HTTP),
        '/ready no consume el limitador',
      );
    });
  },
});
