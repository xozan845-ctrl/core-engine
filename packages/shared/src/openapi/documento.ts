import { mkdirSync, writeFileSync } from 'fs';
import { resolve } from 'path';

/** Superficies del contrato público e interno (R-DO-8). */
export type SuperficieOpenApi = 'publica' | 'interna';

const RE_PUBLICA = /^\/api\/v1(?:\/|$)/;
const RE_INTERNA = /^\/internal(?:\/|$)/;

/**
 * Clasifica una ruta por superficie. Falla si no pertenece a ninguna: una ruta
 * fuera de `/api/v1` o `/internal` rompería la taxonomía del contrato.
 */
export function clasificarRutaOpenApi(ruta: string): SuperficieOpenApi {
  if (RE_PUBLICA.test(ruta)) return 'publica';
  if (RE_INTERNA.test(ruta)) return 'interna';
  throw new Error(
    `ruta fuera de las superficies conocidas: ${ruta} (esperado /api/v1/** o /internal/**)`,
  );
}

type Documento = Record<string, unknown>;

function clonar<T>(valor: T): T {
  return JSON.parse(JSON.stringify(valor)) as T;
}

function refsDe(valor: unknown, salida: Set<string>): void {
  if (Array.isArray(valor)) {
    for (const item of valor) refsDe(item, salida);
    return;
  }
  if (!valor || typeof valor !== 'object') return;
  for (const [clave, hijo] of Object.entries(valor as Record<string, unknown>)) {
    if (clave === '$ref' && typeof hijo === 'string') {
      salida.add(hijo);
      continue;
    }
    refsDe(hijo, salida);
  }
}

function nombreSchema(ref: string): string {
  const prefijo = '#/components/schemas/';
  if (!ref.startsWith(prefijo)) {
    throw new Error(`$ref no soportado en el splitter OpenAPI: ${ref}`);
  }
  return decodeURIComponent(ref.slice(prefijo.length));
}

/**
 * Calcula los schemas alcanzables desde una superficie. Evita que el contrato
 * interno arrastre DTOs que solo usa la API pública (y viceversa).
 */
export function schemasAlcanzables(
  documento: Documento,
  paths: Record<string, unknown>,
): Record<string, unknown> {
  const todos = ((documento.components as Documento | undefined)?.schemas ?? {}) as Record<
    string,
    unknown
  >;
  const pendientes = new Set<string>();
  const incluidos = new Set<string>();
  refsDe(paths, pendientes);
  const cola = [...pendientes].map(nombreSchema);
  while (cola.length > 0) {
    const nombre = cola.shift()!;
    if (incluidos.has(nombre)) continue;
    const schema = todos[nombre];
    if (schema === undefined) {
      throw new Error(`$ref sin schema en components.schemas: ${nombre}`);
    }
    incluidos.add(nombre);
    const refs = new Set<string>();
    refsDe(schema, refs);
    for (const ref of refs) cola.push(nombreSchema(ref));
  }
  return Object.fromEntries(
    [...incluidos].sort().map((nombre) => [nombre, clonar(todos[nombre])]),
  );
}

function separar(
  documento: Documento,
  superficie: SuperficieOpenApi,
  servicio: string,
): Documento {
  const pathsOriginales = (documento.paths ?? {}) as Record<string, unknown>;
  const paths: Record<string, unknown> = {};
  for (const [ruta, operaciones] of Object.entries(pathsOriginales)) {
    if (clasificarRutaOpenApi(ruta) !== superficie) continue;
    paths[ruta] = clonar(operaciones);
  }
  const esPublica = superficie === 'publica';
  const info = (documento.info ?? {}) as Documento;
  return {
    openapi: documento.openapi ?? '3.0.0',
    info: {
      ...info,
      title: `Core Engine · ${servicio} · API ${esPublica ? 'pública' : 'interna'}`,
      description: esPublica
        ? `Contrato público (/api/v1/**) de ${servicio}. No contiene rutas /internal/**.`
        : `Contrato interno (/internal/**) de ${servicio}; requiere x-internal-key.`,
      version: esPublica ? 'v1' : 'internal',
    },
    'x-servicio': servicio,
    'x-superficie': superficie,
    ...(esPublica ? {} : { security: [{ internalKey: [] }] }),
    tags: Array.isArray(documento.tags) ? clonar(documento.tags) : [],
    paths,
    components: {
      securitySchemes: clonar(
        (documento.components as Documento | undefined)?.securitySchemes ?? {},
      ),
      schemas: schemasAlcanzables(documento, paths),
    },
  };
}

/**
 * Separa un documento Nest en sus dos superficies, calculando la clausura de
 * `$ref` para que cada archivo sea autocontenido.
 */
export function separarDocumentoOpenApi(
  documento: object,
  servicio: string,
): { publico: Documento; interno: Documento } {
  const doc = documento as Documento;
  return {
    publico: separar(doc, 'publica', servicio),
    interno: separar(doc, 'interna', servicio),
  };
}

/**
 * Escribe `docs/openapi/<servicio>/public.openapi.json` e
 * `internal.openapi.json` desde el documento generado por Nest.
 */
export function exportarDocumentoOpenApi(
  documento: object,
  servicio: string,
  directorio: string,
): void {
  const destino = resolve(directorio, servicio);
  mkdirSync(destino, { recursive: true });
  const { publico, interno } = separarDocumentoOpenApi(documento, servicio);
  writeFileSync(resolve(destino, 'public.openapi.json'), `${JSON.stringify(publico, null, 2)}\n`);
  writeFileSync(resolve(destino, 'internal.openapi.json'), `${JSON.stringify(interno, null, 2)}\n`);
}
