import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText, walkFiles } from '../../engine/lib/fs.mjs';

/**
 * Agnosticismo de rubro (ADR-0001, R-AR-5).
 *
 * El rubro es configuración (vertical + pack versionado + capacidades), no
 * código. Fuera de identity-service ningún módulo puede ramificar por vertical
 * o pack: solo puede exigir capacidades. Y los rubros nuevos se crean por API
 * (pack builder), no con un despliegue.
 */

const VERTICALES = ['retail', 'farmacia', 'belleza', 'dental', 'flota'];

/** Carpetas que sí conocen el modelo de verticales/packs. */
const EXCLUIDOS = [
  'packages/identity-service/',
  'packages/shared/src/capabilities/',
  'packages/api-gateway/src/openapi.agregado.ts',
];

/** Controladores que deben exigir capacidades en vez de mirar el rubro. */
const GATEADOS_POR_CAPACIDAD = [
  'packages/catalog-service/src/items/items.controller.ts',
  'packages/stores-service/src/ofertas/ofertas.controller.ts',
  'packages/scheduling-service/src/scheduling/scheduling.controller.ts',
  'packages/finance-service/src/facturacion/facturacion.controller.ts',
  'packages/logistics-service/src/envios/estado-orden.controller.ts',
];

const RE_BRANCH = /(?:vertical_id|verticalId|vertical|pack_codigo|packCodigo)\s*(?:===|!==|==|!=)\s*['"`]/;
const RE_SWITCH = /switch\s*\([^)]*(?:vertical|pack)[^)]*\)/i;
const RE_CASE = new RegExp(`case\\s+['"\`](?:pack_)?(?:${VERTICALES.join('|')})['"\`]`);
const RE_LITERAL = new RegExp(`['"\`](?:pack_)?(?:${VERTICALES.join('|')})['"\`]`);

function esComentario(linea) {
  return /^\s*(\/\/|\*|\/\*)/.test(linea);
}

function archivosDePaquetes() {
  return walkFiles(
    join(repoRoot, 'packages'),
    (nombre) =>
      nombre.endsWith('.ts') &&
      !nombre.endsWith('.spec.ts') &&
      !nombre.endsWith('.test.ts'),
  ).filter((archivo) => {
    const rel = archivo.replace(`${repoRoot}/`, '');
    return !EXCLUIDOS.some((excluido) => rel.startsWith(excluido));
  });
}

export default defineSuite({
  id: 'architecture-agnosticismo',
  area: 'microservices',
  rules: ['R-AR-5'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('ADR-0001: fuera de identity no se ramifica por vertical ni por pack', () => {
      const violaciones = [];
      for (const archivo of archivosDePaquetes()) {
        const rel = archivo.replace(`${repoRoot}/`, '');
        readText(archivo)
          .split('\n')
          .forEach((linea, indice) => {
            if (esComentario(linea)) return;
            if (RE_BRANCH.test(linea) || RE_SWITCH.test(linea) || RE_CASE.test(linea)) {
              violaciones.push(`${rel}:${indice + 1}: ${linea.trim()}`);
            }
          });
      }
      assert.deepEqual(
        violaciones,
        [],
        `El rubro debe resolverse por capacidades (ADR-0001), no por vertical/pack:\n${violaciones.join('\n')}`,
      );
    });

    test('ADR-0001: los códigos de rubro solo viven en identity y migraciones', () => {
      const violaciones = [];
      for (const archivo of archivosDePaquetes()) {
        const rel = archivo.replace(`${repoRoot}/`, '');
        if (rel === 'packages/shared/src/constants.ts') continue;
        readText(archivo)
          .split('\n')
          .forEach((linea, indice) => {
            if (esComentario(linea)) return;
            if (RE_LITERAL.test(linea)) {
              violaciones.push(`${rel}:${indice + 1}: ${linea.trim()}`);
            }
          });
      }
      assert.deepEqual(
        violaciones,
        [],
        `Los códigos de vertical/pack no deben codificarse fuera de identity:\n${violaciones.join('\n')}`,
      );
    });

    test('R-AR-5: los servicios de dominio exigen capacidades, no rubro', () => {
      const faltantes = GATEADOS_POR_CAPACIDAD.filter(
        (rel) => !readText(join(repoRoot, rel)).includes('@RequiereCapacidades('),
      );
      assert.deepEqual(
        faltantes,
        [],
        `Controladores sin @RequiereCapacidades (R-AR-5):\n${faltantes.join('\n')}`,
      );
    });

    test('ADR-0001: el pack builder crea verticales y packs con capacidades arbitrarias', () => {
      const builder = readText(
        join(repoRoot, 'packages/identity-service/src/packs/packs.controller.ts'),
      );
      assert.match(builder, /@Post\('verticales'\)/, 'falta POST /packs/verticales');
      assert.match(builder, /@Post\('modelos-operacion'\)/, 'falta POST /packs/modelos-operacion');
      assert.match(builder, /@Post\(\)/, 'falta POST /packs');
      assert.match(builder, /capacidades/, 'el pack no acepta capacidades arbitrarias');
      assert.match(builder, /@Roles\(ROLES\.SUPER_ADMIN, ROLES\.ADMIN\)/, 'el alta de packs debe ser administrativa');
    });
  },
});
