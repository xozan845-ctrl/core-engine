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
];

/**
 * Servicios siempre activos: no representan una capacidad de negocio
 * opcional. identity configura, gateway autentica/enruta, storage guarda
 * objetos y notifications entrega mensajes.
 */
const SERVICIOS_SIEMPRE_ACTIVOS = [
  'packages/identity-service/',
  'packages/api-gateway/',
  'packages/storage-service/',
  'packages/notifications-service/',
];

/**
 * Controladores transversales que no exponen una capacidad de negocio:
 * superficie interna (ClaveInternaGuard), carrito del comprador, tienda
 * publica y perfil fiscal del vendedor.
 */
const CONTROLADORES_TRANSVERSALES = [
  'packages/catalog-service/src/internal/internal.controller.ts',
  'packages/orders-service/src/pedidos/carrito/carrito.controller.ts',
  'packages/orders-service/src/pedidos/interno.controller.ts',
  'packages/stores-service/src/internal/internal.controller.ts',
  'packages/stores-service/src/tiendas/tiendas.controller.ts',
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

    test('R-AR-5: los servicios de feature exigen capacidades, no rubro', () => {
      const faltantes = walkFiles(
        join(repoRoot, 'packages'),
        (nombre) => nombre.endsWith('.controller.ts') && !nombre.endsWith('.spec.ts'),
      )
        .map((archivo) => archivo.replace(`${repoRoot}/`, ''))
        .filter(
          (rel) =>
            !SERVICIOS_SIEMPRE_ACTIVOS.some((prefijo) => rel.startsWith(prefijo)) &&
            !CONTROLADORES_TRANSVERSALES.includes(rel),
        )
        .filter((rel) => !readText(join(repoRoot, rel)).includes('@RequiereCapacidades('));
      assert.deepEqual(
        faltantes,
        [],
        `Controladores sin @RequiereCapacidades (R-AR-5):\n${faltantes.join('\n')}`,
      );
    });

    test('R-AR-5: toda capacidad declarada existe en las migraciones', () => {
      const constantes = readText(
        join(repoRoot, 'packages/shared/src/capabilities/capability.constants.ts'),
      );
      const codigos = [...constantes.matchAll(/:\s*'([a-z_]+)'/g)].map((match) => match[1]);
      assert.ok(codigos.length > 0, 'no se encontraron capacidades declaradas');
      const migraciones = walkFiles(
        join(repoRoot, 'infra/db/init'),
        (nombre) => nombre.endsWith('.sql'),
      )
        .map((archivo) => readText(archivo))
        .join('\n');
      const faltantes = codigos.filter((codigo) => !migraciones.includes(`'${codigo}'`));
      assert.deepEqual(
        faltantes,
        [],
        `Capacidades sin migracion que las registre (R-AR-5):\n${faltantes.join('\n')}`,
      );
    });

    test('R-AR-5: el pack builder mantiene vertical y pack sincronizados', () => {
      const packs = readText(
        join(repoRoot, 'packages/identity-service/src/packs/packs.service.ts'),
      );
      assert.match(
        packs,
        /INSERT INTO identity\.vertical_capacidades/,
        'las capacidades del pack deben quedar disponibles en la vertical (add-ons)',
      );
      assert.match(
        packs,
        /FROM identity\.modelos_operacion/,
        'la vertical debe validar su modelo de operación contra el catálogo',
      );
    });

    test('R-AR-5: el alta de empresa exige vertical y pack explícitos', () => {
      const controller = readText(
        join(repoRoot, 'packages/identity-service/src/empresas/empresas.controller.ts'),
      );
      const inicio = controller.indexOf('class CrearEmpresaRequestDto');
      const fin = controller.indexOf('class ActualizarPackRequestDto');
      const dto = inicio >= 0 && fin > inicio ? controller.slice(inicio, fin) : '';
      assert.ok(dto, 'no se encontró CrearEmpresaRequestDto');
      assert.doesNotMatch(dto, /vertical_id\?/, 'vertical_id no puede ser opcional');
      assert.doesNotMatch(dto, /pack_codigo\?/, 'pack_codigo no puede ser opcional');
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
