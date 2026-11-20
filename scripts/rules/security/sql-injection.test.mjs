import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText, walkFiles } from '../../engine/lib/fs.mjs';

/**
 * Inyección SQL (R-DS-4).
 *
 * En Rust las consultas se parametrizan con `$n`; solo se permite interpolar
 * identificadores internos (tabla/columna/orden) construidos con constantes o
 * allowlists. Se marcan las interpolaciones que huelen a entrada de petición.
 */

const PELIGROSO =
  /(dto|body|query|params?|request|req\.|input|valor|correo|nombre|payload|filtro_usuario)/i;
const PERMITIDOS = [
  /^(tabla|tabla\.nombre|this\.tabla)$/,
  /^(where|where_sql|where_clause|filtro|condiciones|condicionesItem|sets|orden|order_by|extra)$/,
  /^(indice|marcadores|params\.length|valores\.length)$/,
  /^(columnas|COLUMNAS|CAMPOS_)/,
  /^(filtro|filtros)\.(expresion|operador)$/,
  /^(i\.)?(empresa_id|activo|negocio_id)$/,
];

export default defineSuite({
  id: 'security-sql-injection',
  area: 'security',
  rules: ['R-DS-4'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-4: las consultas SQL solo interpolan identificadores internos', () => {
      const archivos = walkFiles(
        join(repoRoot, 'packages'),
        (nombre) => nombre.endsWith('.rs') && !nombre.includes('/target/'),
      );
      assert.ok(
        archivos.length > 0,
        'el escáner no encontró archivos Rust: el check sería vacuo',
      );
      const problemas = [];
      let consultas = 0;

      for (const archivo of archivos) {
        const lineas = readText(archivo).split('\n');
        lineas.forEach((linea, indice) => {
          if (!/sqlx::query|query_scalar|query_as/.test(linea)) return;
          // La consulta puede formatearse en la misma línea o en las dos
          // siguientes; se revisan las interpolaciones del bloque inmediato.
          const bloque = lineas.slice(indice, indice + 4).join('\n');
          if (!bloque.includes('format!')) return;
          consultas += 1;
          for (const match of bloque.matchAll(/\{([^}]+)\}/g)) {
            const expresion = match[1].trim();
            if (PERMITIDOS.some((patron) => patron.test(expresion))) continue;
            if (PELIGROSO.test(expresion)) {
              problemas.push(`${archivo}:${indice + 1}: \${${expresion}}`);
            }
          }
        });
      }

      assert.ok(consultas > 0, 'no se encontró ninguna consulta SQL que auditar');
      assert.deepEqual(
        problemas,
        [],
        `Interpolación SQL con entrada de petición (R-DS-4):\n${problemas.join('\n')}`,
      );
    });
  },
});
