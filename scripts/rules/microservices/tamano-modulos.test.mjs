import { join, relative, sep } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readJson, readText, walkFiles } from '../../engine/lib/fs.mjs';
import { PACKAGES_DIR, SERVICES } from '../../config/project.mjs';

/**
 * R-MS-7: mínima responsabilidad por archivo.
 *
 * Un archivo Rust de `src/` no puede superar ninguno de estos presupuestos:
 *   - 800 líneas de producción (sin el módulo `#[cfg(test)]`),
 *   - 20 funciones públicas (módulos de dominio con SQL; `http/` queda fuera
 *     porque cada handler es una ruta).
 *
 * `scripts/config/module-size-baseline.json` registra la deuda existente. La
 * deuda no puede crecer, y cuando un archivo mejora hay que bajar su entrada
 * para que el baseline funcione como trinquete. Un archivo que ya cumple sale
 * del baseline en el mismo PR.
 */
const METRICAS = ['lines', 'publicFns'];

function metricas(texto) {
  const inicioTests = texto.search(/#\[cfg\(test\)\]/);
  const produccion = inicioTests >= 0 ? texto.slice(0, inicioTests) : texto;
  const tieneSql = /\b(?:FROM|JOIN|INTO|UPDATE|DELETE\s+FROM|INSERT\s+INTO)\s+[a-z_][a-z0-9_]*\./i.test(
    produccion,
  );

  return {
    lines: (produccion.match(/\n/g) ?? []).length,
    publicFns: (produccion.match(/^\s*pub\s+(?:async\s+)?fn\s+\w+/gm) ?? []).length,
    tieneSql,
  };
}

/** Métricas que superan el presupuesto; vacío = el archivo cumple R-MS-7. */
function presupuestosExcedidos(valores, limites, ruta) {
  const excedidos = [];
  if (valores.lines > limites.lines) excedidos.push('lines');
  const aplicaFunciones = !ruta.includes(`${sep}http${sep}`) && valores.tieneSql;
  if (aplicaFunciones && valores.publicFns > limites.publicFns) {
    excedidos.push('publicFns');
  }
  return excedidos;
}

export default defineSuite({
  id: 'microservices-tamano-modulos',
  area: 'microservice',
  rules: ['R-MS-7'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-MS-7: cada archivo respeta los presupuestos de responsabilidad', () => {
      const baseline = readJson(
        join(repoRoot, 'scripts', 'config', 'module-size-baseline.json'),
      );
      const { limits, debt } = baseline;
      const problemas = [];

      for (const servicio of SERVICES) {
        const src = join(repoRoot, PACKAGES_DIR, servicio, 'src');
        for (const archivo of walkFiles(src, (name) => name.endsWith('.rs'))) {
          const ruta = relative(repoRoot, archivo).split(sep).join('/');
          const valores = metricas(readText(archivo));
          const excedidos = presupuestosExcedidos(valores, limits, archivo);

          if (!(ruta in debt)) {
            if (excedidos.length > 0) {
              problemas.push(
                `${ruta}: supera ${excedidos.join(', ')} (${JSON.stringify(valores)}); divide el módulo (R-MS-7)`,
              );
            }
            continue;
          }

          for (const metrica of METRICAS) {
            if (valores[metrica] > debt[ruta][metrica]) {
              problemas.push(
                `${ruta}: ${metrica} creció de ${debt[ruta][metrica]} a ${valores[metrica]}; la deuda de R-MS-7 no puede aumentar`,
              );
            }
          }

          for (const metrica of excedidos) {
            if (valores[metrica] < debt[ruta][metrica]) {
              problemas.push(
                `${ruta}: ${metrica} mejoró de ${debt[ruta][metrica]} a ${valores[metrica]}; baja la entrada del baseline para consolidar la mejora`,
              );
            }
          }
        }
      }

      assert.deepEqual(problemas, [], problemas.join('\n'));
    });

    test('R-MS-7: el baseline solo conserva archivos que siguen excediendo algún presupuesto', () => {
      const baseline = readJson(
        join(repoRoot, 'scripts', 'config', 'module-size-baseline.json'),
      );
      const obsoletos = [];

      for (const ruta of Object.keys(baseline.debt)) {
        let valores;
        try {
          valores = metricas(readText(join(repoRoot, ruta)));
        } catch {
          obsoletos.push(`${ruta}: ya no existe; elimínalo del baseline`);
          continue;
        }

        if (presupuestosExcedidos(valores, baseline.limits, ruta).length === 0) {
          obsoletos.push(
            `${ruta}: ya cumple los presupuestos (${JSON.stringify(valores)}); elimínalo del baseline`,
          );
        }
      }

      assert.deepEqual(obsoletos, [], obsoletos.join('\n'));
    });
  },
});
