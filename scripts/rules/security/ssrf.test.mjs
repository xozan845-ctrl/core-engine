import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText, walkFiles } from '../../engine/lib/fs.mjs';

/**
 * SSRF (R-DS-5).
 *
 * Las llamadas `fetch` deben construirse con URLs internas/configuradas, nunca
 * con un valor que venga directamente de la petición del usuario.
 */

const PROHIBIDOS = ['req.', 'dto.', 'body.', 'query.', 'params.', 'url_invitacion', 'avatar_url'];

export default defineSuite({
  id: 'security-ssrf',
  area: 'security',
  rules: ['R-DS-5'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-5: ningún fetch usa una URL tomada directamente de la petición', () => {
      const archivos = walkFiles(
        join(repoRoot, 'packages'),
        (nombre) =>
          nombre.endsWith('.ts') &&
          !nombre.endsWith('.spec.ts') &&
          !nombre.includes('.stryker-tmp'),
      );
      const problemas = [];

      for (const archivo of archivos) {
        const lineas = readText(archivo).split('\n');
        lineas.forEach((linea, indice) => {
          if (!linea.includes('fetch(')) return;
          if (/^\s*(\/\/|\*)/.test(linea)) return;
          for (const prohibido of PROHIBIDOS) {
            if (linea.includes(prohibido)) {
              problemas.push(`${archivo}:${indice + 1}: ${linea.trim()}`);
            }
          }
        });
      }

      assert.deepEqual(
        problemas,
        [],
        `fetch con datos de la petición (R-DS-5):\n${problemas.join('\n')}`,
      );
    });
  },
});
