import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * Cabeceras de seguridad y CORS (R-GW-2).
 *
 * El gateway debe enviar defensas de navegador en todas las respuestas y el
 * CORS debe usar la whitelist del entorno, nunca reflejar cualquier origen.
 */

const HEADERS = readText(
  join(repoRoot, 'packages/api-gateway/src/proxy/security-headers.middleware.ts'),
);
const MAIN = readText(join(repoRoot, 'packages/api-gateway/src/main.ts'));
const ENV = readText(join(repoRoot, '.env.example'));
const STAGE_ENV = readText(join(repoRoot, '.env.stage.example'));
const PROD_ENV = readText(join(repoRoot, '.env.production.example'));

const REQUERIDAS = [
  'X-Content-Type-Options',
  'X-Frame-Options',
  'Referrer-Policy',
  'Content-Security-Policy',
  'Strict-Transport-Security',
  'Permissions-Policy',
  'Cross-Origin-Opener-Policy',
  'Cross-Origin-Embedder-Policy',
  'Cross-Origin-Resource-Policy',
];

export default defineSuite({
  id: 'security-headers-cors',
  area: 'security',
  rules: ['R-GW-2'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-GW-2: el gateway envía las cabeceras de seguridad', () => {
      for (const cabecera of REQUERIDAS) {
        assert.ok(HEADERS.includes(cabecera), `falta la cabecera ${cabecera}`);
      }
    });

    test('R-GW-2: CORS usa la whitelist del entorno y no refleja cualquier origen', () => {
      assert.ok(MAIN.includes('origin: origenes'), 'CORS debe usar la lista de CORS_ORIGINS');
      assert.ok(
        MAIN.includes('credentials: false'),
        'sin gate de borde no se habilitan credenciales CORS (ADR-0005)',
      );
      assert.ok(!MAIN.includes('origin: true'), 'CORS no debe reflejar cualquier origen');
      assert.ok(!MAIN.includes("origin: '*'"), 'CORS no debe usar wildcard');
    });

    test('R-GW-2: las plantillas de stage y producción no habilitan wildcard CORS', () => {
      assert.ok(ENV.includes('CORS_ORIGINS='), 'falta CORS_ORIGINS en .env.example');
      for (const plantilla of [STAGE_ENV, PROD_ENV]) {
        const lineas = plantilla
          .split('\n')
          .filter((linea) => linea.startsWith('CORS_ORIGINS='));
        assert.ok(lineas.length > 0, 'falta CORS_ORIGINS en la plantilla de entorno');
        for (const linea of lineas) {
          assert.ok(!linea.includes('*'), 'CORS_ORIGINS no debe ser * fuera de desarrollo');
        }
      }
    });
  },
});
