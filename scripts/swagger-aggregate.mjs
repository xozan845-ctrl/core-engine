// Genera el documento OpenAPI agregado que sirve el API Gateway en /docs.
//
// Lee los specs versionados por servicio (docs/openapi/*.json), los fusiona y
// emite un modulo TypeScript generado dentro del gateway para que viaje en la
// imagen Docker (el runtime no copia docs/). Se ejecuta desde
// `npm run swagger:export` y lo verifica el gate G-8 (R-C-7, R-DO-6).
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const raiz = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dirSpecs = resolve(raiz, 'docs/openapi');
const salida = resolve(raiz, 'packages/api-gateway/src/openapi.agregado.ts');

const ficheros = readdirSync(dirSpecs)
  .filter((f) => f.endsWith('.json'))
  .sort();

if (ficheros.length === 0) {
  console.error('swagger-aggregate: no hay specs en docs/openapi (¿faltó swagger:export?)');
  process.exit(1);
}

const paths = {};
const schemas = {};
const securitySchemes = {};
const tags = new Map();
const origenSchema = new Map();
const colisiones = [];

for (const fichero of ficheros) {
  const spec = JSON.parse(readFileSync(resolve(dirSpecs, fichero), 'utf8'));

  for (const tag of spec.tags ?? []) {
    if (!tags.has(tag.name)) tags.set(tag.name, tag);
  }

  for (const [ruta, operaciones] of Object.entries(spec.paths ?? {})) {
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
  console.error('swagger-aggregate: colisiones detectadas (hay que desambiguar):');
  for (const c of colisiones) console.error(`  - ${c}`);
  process.exit(1);
}

const documento = {
  openapi: '3.0.0',
  info: {
    title: 'Core Engine · API (agregada)',
    description:
      'Documento agregado de todos los microservicios, servido por el API Gateway. ' +
      'Cada operacion se enruta a traves del gateway (prefijo /api/v1).',
    version: 'v1',
  },
  tags: [...tags.values()].sort((a, b) => String(a.name).localeCompare(String(b.name))),
  paths,
  components: {
    securitySchemes:
      Object.keys(securitySchemes).length > 0
        ? securitySchemes
        : { bearer: { type: 'http', scheme: 'bearer', bearerFormat: 'JWT' } },
    schemas,
  },
};

const cabecera = `/**
 * GENERADO por scripts/swagger-aggregate.mjs — NO editar a mano.
 * Fuente: docs/openapi/*.json (npm run swagger:export). Verifica: gate G-8.
 */
import type { OpenAPIObject } from '@nestjs/swagger';

export const OPENAPI_AGREGADO = `;

writeFileSync(salida, `${cabecera}${JSON.stringify(documento, null, 2)} as unknown as OpenAPIObject;\n`);

const nPaths = Object.keys(paths).length;
console.log(
  `swagger-aggregate: ${ficheros.length} specs -> ${nPaths} rutas, ${Object.keys(schemas).length} schemas, ${tags.size} tags -> ${salida.replace(`${raiz}/`, '')}`,
);
