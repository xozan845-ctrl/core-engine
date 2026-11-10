import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText, walkFiles } from '../../engine/lib/fs.mjs';

/**
 * Logs con datos sensibles (R-OB-1 / R-DS-2).
 *
 * Los logs estructurados no deben incluir contraseñas, tokens ni cabeceras de
 * autorización. Se revisan solo las líneas que emiten logs.
 */

const SENSIBLES = /(password|contrasena|refresh_token|access_token|authorization|secret|api[_-]?key|internal[_-]?key)/i;
const EMITE = /(logger|console)\.(info|warn|error|log|debug)\s*\(/;

export default defineSuite({
  id: 'security-sensitive-logs',
  area: 'security',
  rules: ['R-OB-1', 'R-DS-2'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-OB-1/R-DS-2: ningún log emite contraseñas ni tokens', () => {
      const archivos = walkFiles(
        join(repoRoot, 'packages'),
        (nombre) =>
          nombre.endsWith('.ts') &&
          !nombre.endsWith('.spec.ts') &&
          !nombre.includes('.stryker-tmp'),
      );
      const problemas = [];

      for (const archivo of archivos) {
        readText(archivo)
          .split('\n')
          .forEach((linea, indice) => {
            if (/^\s*(\/\/|\*)/.test(linea)) return;
            if (!EMITE.test(linea)) return;
            if (SENSIBLES.test(linea)) {
              problemas.push(`${archivo}:${indice + 1}: ${linea.trim()}`);
            }
          });
      }

      assert.deepEqual(
        problemas,
        [],
        `Log con datos sensibles (R-OB-1/R-DS-2):\n${problemas.join('\n')}`,
      );
    });
  },
});
