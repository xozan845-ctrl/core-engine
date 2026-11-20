import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText, walkFiles } from '../../engine/lib/fs.mjs';

/**
 * Logs con datos sensibles (R-OB-1 / R-DS-2).
 *
 * Los logs estructurados no deben incluir contraseñas, tokens ni cabeceras de
 * autorización. Se revisan las líneas que emiten logs en el código Rust.
 */

const SENSIBLES =
  /(password|contrasena|contraseña|refresh_token|access_token|authorization|secret|api[_-]?key|internal[_-]?key|bearer)/i;
const EMITE =
  /(logs::(info|warn|error|debug|trace)|tracing::(info|warn|error|debug|trace)|println!|eprintln!|dbg!)\s*\(/;

export default defineSuite({
  id: 'security-sensitive-logs',
  area: 'security',
  rules: ['R-OB-1', 'R-DS-2'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-OB-1/R-DS-2: ningún log emite contraseñas ni tokens', () => {
      const archivos = walkFiles(
        join(repoRoot, 'packages'),
        (nombre) => nombre.endsWith('.rs') && !nombre.includes('/target/'),
      );
      assert.ok(
        archivos.length > 0,
        'el escáner no encontró archivos Rust: el check sería vacuo',
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
