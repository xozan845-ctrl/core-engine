import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * OWASP API3:2023 — Excessive Data Exposure (R-DS-3 / R-C-3).
 *
 * Las proyecciones públicas del catálogo y del storefront deben ser listas
 * blancas. Un campo interno (costo, margen, stock exacto, IDs de usuario,
 * reparto) no puede declararse en `*.publico.ts`; si alguien lo agrega, esta
 * suite falla y señala el campo.
 */

const CATALOGO = join(
  repoRoot,
  'packages/catalog-service/src/productos/productos.publico.ts',
);
const CATALOGO_CONTROLLER = join(
  repoRoot,
  'packages/catalog-service/src/productos/productos.controller.ts',
);
const STOREFRONT = join(
  repoRoot,
  'packages/stores-service/src/ofertas/ofertas.publico.ts',
);
const STOREFRONT_CONTROLLER = join(
  repoRoot,
  'packages/stores-service/src/ofertas/ofertas.controller.ts',
);

/** Campos que jamás deben declararse en la proyección pública del catálogo. */
const SENSIBLES_CATALOGO = [
  'costo',
  'tipo_costo',
  'stock',
  'stock_total',
  'stock_minimo',
  'stock_maximo',
  'codigo_barras',
  'ubicacion',
  'es_materia_prima',
  'estado_operativo',
];

/** Campos internos de tienda/oferta que no deben publicarse. */
const SENSIBLES_STOREFRONT = [
  'margen',
  'precio_base',
  'stock',
  'participacion_porcentaje',
  'propietario_usuario_id',
  'vendedor_id',
  'tienda_id',
];

/**
 * Detecta una declaración/asignación de propiedad, no una mención en un
 * comentario. Ignora `campo: _alias`, que es el destructuring usado para
 * excluir el campo de la proyección pública (no lo publica).
 */
function declaraCampo(texto, campo) {
  return new RegExp(`^\\s*${campo}\\??\\s*:(?!\\s*_)`, 'm').test(texto);
}

export default defineSuite({
  id: 'security-excessive-data-exposure',
  area: 'security',
  rules: ['R-DS-3', 'R-C-3'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-3: el catálogo público no declara campos comerciales internos', () => {
      const texto = readText(CATALOGO);
      for (const campo of SENSIBLES_CATALOGO) {
        assert.equal(
          declaraCampo(texto, campo),
          false,
          `productos.publico.ts publica "${campo}" (R-DS-3)`,
        );
      }
      assert.match(texto, /disponible/, 'la proyección pública debe exponer "disponible"');
    });

    test('R-C-3: las lecturas públicas del catálogo usan la proyección', () => {
      const texto = readText(CATALOGO_CONTROLLER);
      assert.match(texto, /aProductoPublico/, 'el controlador no aplica aProductoPublico (R-C-3)');
      assert.match(
        texto,
        /esPropietarioCatalogo/,
        'el controlador no distingue propietario/admin de lectura pública (R-C-3)',
      );
    });

    test('R-DS-3: la tienda/oferta pública no declara campos internos', () => {
      const texto = readText(STOREFRONT);
      for (const campo of SENSIBLES_STOREFRONT) {
        assert.equal(
          declaraCampo(texto, campo),
          false,
          `ofertas.publico.ts publica "${campo}" (R-DS-3)`,
        );
      }
      assert.match(texto, /disponible/, 'la oferta pública debe exponer "disponible"');
    });

    test('R-C-3: el storefront público usa la proyección', () => {
      const texto = readText(STOREFRONT_CONTROLLER);
      assert.match(texto, /aTiendaPublica/, 'el controlador no aplica aTiendaPublica (R-C-3)');
      assert.match(texto, /aOfertaPublica/, 'el controlador no aplica aOfertaPublica (R-C-3)');
      assert.match(texto, /esDueno/, 'el controlador no distingue dueño/admin (R-C-3)');
    });
  },
});
