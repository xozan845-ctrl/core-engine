import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * OWASP API3:2023 — Excessive Data Exposure (R-DS-3 / R-C-3).
 *
 * El storefront público (GET /api/v2/tiendas/:id) no lleva autenticación y debe
 * servirse con listas blancas. Un campo comercial interno (margen, precio base,
 * participación del vendedor, ids de usuario) no puede aparecer en la
 * proyección pública; si alguien lo agrega, esta suite falla y señala el campo.
 */

const PUBLICO = join(repoRoot, 'packages/stores-service/src/ofertas/publico.rs');
const CONTROLLER = join(
  repoRoot,
  'packages/stores-service/src/http/tiendas.rs',
);
const CATALOGO_DTO = join(
  repoRoot,
  'packages/catalog-service/src/items/mod.rs',
);

/** Campos que jamás deben publicarse en el storefront. */
const SENSIBLES_STOREFRONT = [
  'margen',
  'precio_base',
  'participacion_porcentaje',
  'propietario_usuario_id',
  'vendedor_id',
  'empresa_id',
  'tienda_id',
];

/** Campos de proveedor/abastecimiento que no pertenecen a una respuesta pública. */
const SENSIBLES_CATALOGO = [
  'proveedor',
  'proveedor_id',
  'codigo_proveedor',
  'notas_internas',
  'costo_promedio',
];

/**
 * Detecta una declaración/asignación de propiedad, no una mención en un
 * comentario.
 */
function declaraCampo(texto, campo) {
  return new RegExp(`^\\s*(?:pub\\s+)?${campo}\\??\\s*:`, 'm').test(texto);
}

export default defineSuite({
  id: 'security-excessive-data-exposure',
  area: 'security',
  rules: ['R-DS-3', 'R-C-3'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-3: la proyección pública del storefront no declara campos internos', () => {
      const texto = readText(PUBLICO);
      for (const campo of SENSIBLES_STOREFRONT) {
        assert.equal(
          declaraCampo(texto, campo),
          false,
          `publico.ts publica "${campo}" (R-DS-3)`,
        );
      }
    });

    test('R-C-3: la ruta pública del storefront usa las proyecciones', () => {
      const texto = readText(CONTROLLER);
      assert.match(
        texto,
        /a_tienda_publica\(/,
        'el controlador no aplica a_tienda_publica (R-C-3)',
      );
      assert.match(
        texto,
        /a_oferta_publica/,
        'el controlador no aplica a_oferta_publica (R-C-3)',
      );
      assert.match(
        texto,
        /ofertas[\s\S]*\.map\(crate::ofertas::a_oferta_publica\)/,
        'las ofertas públicas deben mapearse una a una (R-C-3)',
      );
    });

    test('R-DS-3: el catálogo no declara campos de proveedor en su DTO', () => {
      const texto = readText(CATALOGO_DTO);
      for (const campo of SENSIBLES_CATALOGO) {
        assert.equal(
          declaraCampo(texto, campo),
          false,
          `items.response.dto.ts publica "${campo}" (R-DS-3)`,
        );
      }
    });
  },
});
