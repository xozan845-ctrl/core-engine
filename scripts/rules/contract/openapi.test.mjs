import { join, relative } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readJson, readText, walkFiles } from '../../engine/lib/fs.mjs';

/**
 * Regla R-DO-8: taxonomía y superficies del contrato OpenAPI.
 *
 * La API tiene dos superficies deliberadamente separadas:
 *   - pública: /api/v1/**  (expuesta por el gateway)
 *   - interna: /internal/** (servicio→servicio, con ClaveInternaGuard)
 *
 * El contrato público no puede anunciar rutas internas y el interno no puede
 * anunciar rutas públicas. Además, todo controlador interno debe llevar la
 * guarda de clave interna (R-DS-5).
 */

const RE_SUPERFICIE = /^(?:api\/v1|internal)(?:\/|$)/;
const RE_CONTROLLER = /@Controller\(\s*(['"`])([^'"`]+)\1\s*\)/g;
const RE_RUTA = /@(Get|Post|Patch|Put|Delete)\(\s*(?:['"`]([^'"`]*)['"`])?\s*\)/g;

const controladores = walkFiles(
  join(repoRoot, 'packages'),
  (nombre) => nombre.endsWith('.controller.ts'),
);

function prefijosDe(texto) {
  return [...texto.matchAll(RE_CONTROLLER)].map((match) => match[2]);
}

/** Divide el archivo por controlador y devuelve sus rutas en orden de declaración. */
function rutasPorControlador(texto) {
  const cabeceras = [...texto.matchAll(RE_CONTROLLER)];
  return cabeceras.map((cabecera, indice) => {
    const inicio = cabecera.index + cabecera[0].length;
    const fin = indice + 1 < cabeceras.length ? cabeceras[indice + 1].index : texto.length;
    const cuerpo = texto.slice(inicio, fin);
    const rutas = [...cuerpo.matchAll(RE_RUTA)].map((ruta) => ({
      metodo: ruta[1].toUpperCase(),
      ruta: ruta[2] ?? '',
    }));
    return { prefijo: cabecera[2], rutas };
  });
}

const segmentos = (ruta) => ruta.split('/').filter(Boolean);

/** Detecta una ruta estática posterior que cae bajo una ruta paramétrica previa. */
function rutasTapadas(prefijo, rutas) {
  const tapadas = [];
  for (let i = 0; i < rutas.length; i += 1) {
    const anterior = rutas[i];
    const segmentosAnterior = segmentos(anterior.ruta);
    if (!segmentosAnterior.some((segmento) => segmento.startsWith(':'))) continue;

    for (let j = i + 1; j < rutas.length; j += 1) {
      const posterior = rutas[j];
      if (posterior.metodo !== anterior.metodo) continue;
      const segmentosPosterior = segmentos(posterior.ruta);
      if (segmentosAnterior.length !== segmentosPosterior.length) continue;

      const coincide = segmentosAnterior.every(
        (segmento, indice) =>
          segmento.startsWith(':') || segmento === segmentosPosterior[indice],
      );
      const esEstatica = !segmentosPosterior.some((segmento) => segmento.startsWith(':'));
      if (coincide && esEstatica) {
        tapadas.push(
          `${anterior.metodo} ${prefijo}/${anterior.ruta} tapa ` +
            `${posterior.metodo} ${prefijo}/${posterior.ruta}`,
        );
      }
    }
  }
  return tapadas;
}

export default defineSuite({
  id: 'openapi-contract',
  area: 'documentation',
  rules: ['R-DO-8', 'R-DO-9', 'R-DS-5'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DO-8: cada controlador declara una superficie /api/v1 o /internal', () => {
      const invalidos = [];
      const sinControlador = [];

      for (const archivo of controladores) {
        const prefijos = prefijosDe(readText(archivo));
        if (prefijos.length === 0) {
          sinControlador.push(relative(repoRoot, archivo));
          continue;
        }
        for (const prefijo of prefijos) {
          if (!RE_SUPERFICIE.test(prefijo)) {
            invalidos.push(`${relative(repoRoot, archivo)}: @Controller('${prefijo}')`);
          }
        }
      }

      assert.deepEqual(
        sinControlador,
        [],
        `Controladores sin @Controller (R-DO-8):\n${sinControlador.join('\n')}`,
      );
      assert.deepEqual(
        invalidos,
        [],
        `Rutas fuera de las superficies /api/v1 o /internal (R-DO-8):\n${invalidos.join('\n')}`,
      );
    });

    test('R-DO-8: el contrato público no contiene rutas /internal', () => {
      const documento = readJson(
        join(repoRoot, 'docs/api/core-engine.openapi.json'),
      );
      const internas = Object.keys(documento.paths).filter((ruta) =>
        ruta.startsWith('/internal'),
      );

      assert.deepEqual(
        internas,
        [],
        `El contrato público mezcla rutas internas (R-DO-8):\n${internas.join('\n')}`,
      );
    });

    test('R-DO-8: el contrato interno no contiene rutas /api/v1', () => {
      const documento = readJson(
        join(repoRoot, 'docs/api/core-engine.internal.openapi.json'),
      );
      const publicas = Object.keys(documento.paths).filter((ruta) =>
        ruta.startsWith('/api/v1'),
      );

      assert.deepEqual(
        publicas,
        [],
        `El contrato interno mezcla rutas públicas (R-DO-8):\n${publicas.join('\n')}`,
      );
    });

    test('R-DO-9: ninguna ruta estática queda tapada por una ruta con parámetro', () => {
      const tapadas = [];

      for (const archivo of controladores) {
        for (const { prefijo, rutas } of rutasPorControlador(readText(archivo))) {
          for (const tapada of rutasTapadas(prefijo, rutas)) {
            tapadas.push(`${relative(repoRoot, archivo)}: ${tapada}`);
          }
        }
      }

      assert.deepEqual(
        tapadas,
        [],
        `Rutas estáticas tapadas por un :param previo (R-DO-9):\n${tapadas.join('\n')}`,
      );
    });

    test('R-DS-5: todo controlador interno usa ClaveInternaGuard', () => {
      const sinGuardia = [];

      for (const archivo of controladores) {
        const texto = readText(archivo);
        const tieneInternas = prefijosDe(texto).some((prefijo) =>
          prefijo.startsWith('internal'),
        );
        if (tieneInternas && !texto.includes('ClaveInternaGuard')) {
          sinGuardia.push(relative(repoRoot, archivo));
        }
      }

      assert.deepEqual(
        sinGuardia,
        [],
        `Controladores /internal sin ClaveInternaGuard (R-DS-5):\n${sinGuardia.join('\n')}`,
      );
    });
  },
});
