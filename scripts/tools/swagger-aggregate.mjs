// Genera los documentos OpenAPI agregados del proyecto.
//
// Hay dos superficies de API distintas y no deben mezclarse:
//   - pública:  /api/v1/**  (expuesta por el API Gateway, versionada)
//   - interna:  /internal/** (servicio→servicio, protegida con x-internal-key)
//
// El contrato público es el que se sirve en /docs y se reparte al frontend.
// El contrato interno se versiona aparte para documentar las llamadas entre
// servicios sin presentarlas como rutas alcanzables desde fuera.
//
// Se ejecuta desde `npm run swagger:export` y lo verifica el gate G-8
// (R-C-7, R-DO-6).
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { repoRoot as raiz } from '../engine/lib/paths.mjs';

const dirSpecs = resolve(raiz, 'docs/openapi');
const salidaGateway = resolve(raiz, 'packages/api-gateway/src/openapi.agregado.ts');
const salidaPublica = resolve(raiz, 'docs/api/core-engine.openapi.json');
const salidaInterna = resolve(raiz, 'docs/api/core-engine.internal.openapi.json');

const RE_PUBLICA = /^\/api\/v1(?:\/|$)/;
const RE_INTERNA = /^\/internal(?:\/|$)/;

/**
 * Clasifica una ruta por superficie. Falla si no pertenece a ninguna: una ruta
 * fuera de `/api/v1` o `/internal` rompería la taxonomía del contrato.
 *
 * @param {string} ruta
 * @returns {'publica'|'interna'}
 */
export function clasificarRuta(ruta) {
  if (RE_PUBLICA.test(ruta)) return 'publica';
  if (RE_INTERNA.test(ruta)) return 'interna';
  throw new Error(
    `ruta fuera de las superficies conocidas: ${ruta} (esperado /api/v1/** o /internal/**)`,
  );
}

/**
 * Lee los specs versionados por servicio.
 *
 * @param {string} [directorio]
 * @returns {{ fichero: string, spec: object }[]}
 */
export function cargarEspecificaciones(directorio = dirSpecs) {
  const ficheros = readdirSync(directorio)
    .filter((f) => f.endsWith('.json'))
    .sort();

  if (ficheros.length === 0) {
    throw new Error('no hay specs en docs/openapi (¿faltó swagger:export?)');
  }

  return ficheros.map((fichero) => ({
    fichero,
    spec: JSON.parse(readFileSync(resolve(directorio, fichero), 'utf8')),
  }));
}

/**
 * Construye un documento agregado para una sola superficie.
 *
 * @param {{ fichero: string, spec: object }[]} especificaciones
 * @param {'publica'|'interna'} superficie
 * @returns {object}
 */
export function construirDocumento(especificaciones, superficie) {
  const paths = {};
  const schemas = {};
  const securitySchemes = {};
  const tags = new Map();
  const origenSchema = new Map();
  const colisiones = [];

  for (const { fichero, spec } of especificaciones) {
    const rutasDeLaSuperficie = Object.entries(spec.paths ?? {}).filter(
      ([ruta]) => clasificarRuta(ruta) === superficie,
    );
    if (rutasDeLaSuperficie.length === 0) continue;

    for (const tag of spec.tags ?? []) {
      if (!tags.has(tag.name)) tags.set(tag.name, tag);
    }

    for (const [ruta, operaciones] of rutasDeLaSuperficie) {
      for (const [metodo, operacion] of Object.entries(operaciones)) {
        if (paths[ruta]?.[metodo]) {
          colisiones.push(`path ${metodo.toUpperCase()} ${ruta} (${fichero})`);
          continue;
        }
        paths[ruta] = paths[ruta] ?? {};
        paths[ruta][metodo] = operacion;
      }
    }

    for (const [nombre, esquema] of Object.entries(spec.components?.schemas ?? {})) {
      const yaExiste = schemas[nombre];
      if (yaExiste && JSON.stringify(yaExiste) !== JSON.stringify(esquema)) {
        colisiones.push(`schema ${nombre}: ${origenSchema.get(nombre)} vs ${fichero}`);
        continue;
      }
      if (!yaExiste) {
        schemas[nombre] = esquema;
        origenSchema.set(nombre, fichero);
      }
    }

    for (const [nombre, esquema] of Object.entries(spec.components?.securitySchemes ?? {})) {
      securitySchemes[nombre] = securitySchemes[nombre] ?? esquema;
    }
  }

  if (colisiones.length > 0) {
    throw new Error(
      `colisiones detectadas (hay que desambiguar):\n${colisiones.map((c) => `  - ${c}`).join('\n')}`,
    );
  }

  const esPublica = superficie === 'publica';
  const documentoSeguridad = esPublica
    ? Object.keys(securitySchemes).length > 0
      ? securitySchemes
      : { bearer: { type: 'http', scheme: 'bearer', bearerFormat: 'JWT' } }
    : {
        internalKey: { type: 'apiKey', in: 'header', name: 'x-internal-key' },
      };

  return {
    openapi: '3.0.0',
    info: {
      title: esPublica
        ? 'Core Engine · API pública (agregada)'
        : 'Core Engine · API interna (servicio→servicio)',
      description: esPublica
        ? 'Contrato de la superficie pública (/api/v1/**) expuesta por el API Gateway. ' +
          'No incluye las rutas /internal/**, que son exclusivas del tráfico servicio→servicio.'
        : 'Contrato de la superficie interna (/internal/**). No se expone a través del API Gateway ' +
          'y requiere la cabecera x-internal-key.',
      version: esPublica ? 'v1' : 'internal',
    },
    ...(esPublica ? {} : { security: [{ internalKey: [] }] }),
    tags: [...tags.values()].sort((a, b) => String(a.name).localeCompare(String(b.name))),
    paths,
    components: {
      securitySchemes: documentoSeguridad,
      schemas,
    },
  };
}

/**
 * Construye ambos documentos a partir de los specs.
 *
 * @param {{ fichero: string, spec: object }[]} especificaciones
 * @returns {{ publica: object, interna: object }}
 */
export function construirDocumentos(especificaciones) {
  return {
    publica: construirDocumento(especificaciones, 'publica'),
    interna: construirDocumento(especificaciones, 'interna'),
  };
}

/** Escribe los artefactos generados. */
export function escribirDocumentos(documentos) {
  const cabecera = `/**
 * GENERADO por scripts/tools/swagger-aggregate.mjs — NO editar a mano.
 * Fuente: docs/openapi/*.json (npm run swagger:export). Verifica: gate G-8.
 */
import type { OpenAPIObject } from '@nestjs/swagger';

export const OPENAPI_AGREGADO = `;

  writeFileSync(salidaGateway, `${cabecera}${JSON.stringify(documentos.publica, null, 2)} as unknown as OpenAPIObject;\n`);

  mkdirSync(resolve(raiz, 'docs/api'), { recursive: true });
  writeFileSync(salidaPublica, `${JSON.stringify(documentos.publica, null, 2)}\n`);
  writeFileSync(salidaInterna, `${JSON.stringify(documentos.interna, null, 2)}\n`);
}

/** Ejecuta la agregación completa. Devuelve el código de salida. */
export function main() {
  try {
    const especificaciones = cargarEspecificaciones();
    const documentos = construirDocumentos(especificaciones);
    escribirDocumentos(documentos);

    const rutas = (documento) => Object.keys(documento.paths).length;
    const operaciones = (documento) =>
      Object.values(documento.paths).reduce((total, ops) => total + Object.keys(ops).length, 0);

    console.log(
      `swagger-aggregate: ${especificaciones.length} specs -> ` +
        `pública ${rutas(documentos.publica)} rutas/${operaciones(documentos.publica)} ops, ` +
        `interna ${rutas(documentos.interna)} rutas/${operaciones(documentos.interna)} ops -> ` +
        `${salidaGateway.replace(`${raiz}/`, '')} (+ ${salidaPublica.replace(`${raiz}/`, '')}, ` +
        `${salidaInterna.replace(`${raiz}/`, '')})`,
    );
    return 0;
  } catch (err) {
    console.error(`swagger-aggregate: ${err instanceof Error ? err.message : String(err)}`);
    return 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = main();
}
