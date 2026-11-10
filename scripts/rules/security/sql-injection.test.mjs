import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText, walkFiles } from '../../engine/lib/fs.mjs';

/**
 * Inyección SQL (R-DS-4).
 *
 * Las consultas deben parametrizarse. Solo se permite interpolar identificadores
 * internos (nombres de columna/tabla fijos, `$n` y fragmentos construidos con
 * placeholders), nunca valores de entrada.
 */

const PERMITIDOS = [
  /^sets\.join\(/,
  /^condiciones(Item)?\.join\(/,
  /^where$/,
  /^tabla$/,
  /^this\.tabla$/,
  /^orderBy$/,
  /^extra$/,
  /^condicion$/,
  /^filtro$/,
  /^marcadores$/,
  /^params\.length/,
  /^COLUMNAS/,
  /^CAMPOS_/,
  /^SELECT_INVITACION$/,
  /^(excluirId|incluirInactivos|origen)\b.*\?/,
  // Fragmentos condicionales fijos: el valor sigue viajando por placeholder.
  /^(empresaId|activo)\s*\?/,
  /^valores\.length$/,
];

const RE_CONSULTA = /\.(?:query|queryOne)(?:<[^>]+>)?\(\s*`([\s\S]*?)`/g;
const RE_INTERPOLACION = /\$\{([^}]*)\}/g;

export default defineSuite({
  id: 'security-sql-injection',
  area: 'security',
  rules: ['R-DS-4'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-4: las consultas SQL solo interpolan identificadores internos', () => {
      const archivos = walkFiles(
        join(repoRoot, 'packages'),
        (nombre) =>
          nombre.endsWith('.ts') &&
          !nombre.endsWith('.spec.ts') &&
          !nombre.includes('.stryker-tmp'),
      );
      const problemas = [];

      for (const archivo of archivos) {
        const texto = readText(archivo);
        for (const consulta of texto.matchAll(RE_CONSULTA)) {
          for (const interpolacion of consulta[1].matchAll(RE_INTERPOLACION)) {
            const expresion = interpolacion[1].trim();
            if (!PERMITIDOS.some((patron) => patron.test(expresion))) {
              problemas.push(`${archivo}: \${${expresion}}`);
            }
          }
        }
      }

      assert.deepEqual(
        problemas,
        [],
        `Interpolación SQL no permitida (R-DS-4):\n${problemas.join('\n')}`,
      );
    });
  },
});
