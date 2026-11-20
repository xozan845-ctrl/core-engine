import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText, walkFiles } from '../../engine/lib/fs.mjs';

/**
 * SSRF en llamadas salientes (R-DS-5).
 *
 * El webhook saliente acepta una URL de tenant: debe bloquear loopback,
 * rangos privados/link-local y redirecciones. El resto de clientes usan URLs
 * de configuración, no de la petición.
 */

const WEBHOOKS = join(
  repoRoot,
  'packages/notifications-service/src/webhooks.rs',
);

export default defineSuite({
  id: 'security-ssrf',
  area: 'security',
  rules: ['R-DS-5'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-5: el webhook bloquea loopback, privadas y redirecciones', () => {
      const fuente = readText(WEBHOOKS);
      assert.match(fuente, /is_loopback/, 'no bloquea loopback');
      assert.match(fuente, /is_private/, 'no bloquea rangos privados IPv4');
      assert.match(fuente, /is_link_local/, 'no bloquea link-local');
      assert.match(
        fuente,
        /redirect\(reqwest::redirect::Policy::none\(\)\)/,
        'el cliente sigue redirecciones',
      );
      assert.match(
        fuente,
        /self\.validar_url\(url\)/,
        'la URL no se valida antes de persistir/entregar',
      );
    });

    test('R-DS-5: ningún cliente sale con URL tomada directamente de la petición', () => {
      const archivos = walkFiles(
        join(repoRoot, 'packages'),
        (nombre) => nombre.endsWith('.rs') && !nombre.includes('/target/'),
      );
      assert.ok(archivos.length > 0, 'el escáner no encontró archivos Rust');
      const problemas = [];
      for (const archivo of archivos) {
        const texto = readText(archivo);
        if (!/reqwest::Client|reqwest::get|\.post\(/.test(texto)) continue;
        for (const [indice, linea] of texto.split('\n').entries()) {
          if (!/(\.post\(|reqwest::get\(|self\.(cliente|http)\.get\()/.test(linea)) continue;
          if (/(dto|body|query|params|request)\./.test(linea)) {
            problemas.push(`${archivo}:${indice + 1}: ${linea.trim()}`);
          }
        }
      }
      assert.deepEqual(
        problemas,
        [],
        `URL de salida tomada de la petición (R-DS-5):\n${problemas.join('\n')}`,
      );
    });
  },
});
