/**
 * config/services.mjs — Catálogo de servicios derivado de los manifiestos.
 * ──────────────────────────────────────────────────────────────────────────────
 * Cada servicio declara su contrato en `packages/<servicio>/service.json`
 * (R-MS-9). Este módulo es la **fuente única de verdad** para:
 *
 *   - la lista de servicios desplegables (`SERVICES`),
 *   - los puertos y esquemas que posee cada uno,
 *   - el perfil arquitectónico y las capacidades que implementa,
 *   - los umbrales de cobertura que consume la matriz de CI.
 *
 * Ningún script debe volver a hardcodear esas listas. `project.mjs` reexporta
 * los valores para conservar la API histórica del motor.
 *
 * Capa `config/`: sólo Node built-ins. No importa `engine/`, `rules/` ni
 * `tools/` (ver scripts/README.md).
 */
import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const CONFIG_DIR = dirname(fileURLToPath(import.meta.url));
export const REPO_ROOT = resolve(CONFIG_DIR, '..', '..');
export const PACKAGES_DIR = 'packages';
export const SERVICE_MANIFEST_FILE = 'service.json';

export const PROFILES = Object.freeze([
  'edge', // perímetro: gateway, sin base de datos
  'core', // servicio síncrono con BD, sin broker
  'transactional', // publica eventos: outbox obligatorio
  'consumer', // consume eventos, sin outbox propio
  'olap', // analítica/lecturas: proyecciones, sin mutaciones de negocio
]);

const ENVIRONMENTS = Object.freeze(['dev', 'stage', 'prod']);

/**
 * Error de configuración del catálogo. Falla el arranque de cualquier script
 * con un mensaje accionable en vez de un `JSON.parse` críptico.
 */
export class ServiceCatalogError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ServiceCatalogError';
  }
}

function readJson(file) {
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch (error) {
    throw new ServiceCatalogError(
      `${file}: no se pudo leer el manifiesto (${error.message})`,
    );
  }
}

function packageDirs() {
  const root = join(REPO_ROOT, PACKAGES_DIR);
  if (!existsSync(root)) return [];
  return readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .filter((name) => existsSync(join(root, name, 'Dockerfile')))
    .sort((a, b) => a.localeCompare(b));
}

function assertString(value, path) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new ServiceCatalogError(`${path}: se esperaba un string no vacío`);
  }
}

function assertStringArray(value, path) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== 'string')) {
    throw new ServiceCatalogError(`${path}: se esperaba un array de strings`);
  }
}

/**
 * Validación estructural del manifiesto. La suite de conformidad añade las
 * comprobaciones contra el repositorio (puerto, esquema, compose, artefactos).
 * @param {unknown} manifest
 * @param {string} source
 * @returns {string[]} errores encontrados (vacío = válido)
 */
export function validateManifestShape(manifest, source = 'service.json') {
  const errors = [];
  const push = (message) => errors.push(`${source}: ${message}`);

  if (!manifest || typeof manifest !== 'object' || Array.isArray(manifest)) {
    return [`${source}: el manifiesto debe ser un objeto JSON`];
  }
  if (manifest.apiVersion !== 'core-engine.io/v1') {
    push('apiVersion debe ser "core-engine.io/v1"');
  }
  if (manifest.kind !== 'Service') push('kind debe ser "Service"');

  const metadata = manifest.metadata;
  if (!metadata || typeof metadata !== 'object') {
    push('falta metadata');
  } else {
    for (const key of ['name', 'boundedContext', 'description', 'owner', 'lifecycle']) {
      try {
        assertString(metadata[key], `metadata.${key}`);
      } catch (error) {
        push(error.message);
      }
    }
    try {
      assertStringArray(metadata.tags, 'metadata.tags');
    } catch (error) {
      push(error.message);
    }
  }

  const spec = manifest.spec;
  if (!spec || typeof spec !== 'object') {
    push('falta spec');
    return errors;
  }

  if (!PROFILES.includes(spec.profile)) {
    push(`spec.profile debe ser uno de: ${PROFILES.join(' | ')}`);
  }
  if (spec.runtime !== 'rust') push('spec.runtime debe ser "rust"');
  if (!Number.isInteger(spec.port) || spec.port < 1 || spec.port > 65535) {
    push('spec.port debe ser un puerto TCP válido');
  }
  if (spec.schema !== null && typeof spec.schema !== 'string') {
    push('spec.schema debe ser string o null');
  }
  if (spec.outboxTable !== null && typeof spec.outboxTable !== 'string') {
    push('spec.outboxTable debe ser string o null');
  }
  if (spec.composeName !== undefined && typeof spec.composeName !== 'string') {
    push('spec.composeName debe ser string cuando existe');
  }
  if (typeof spec.publicApi !== 'boolean') push('spec.publicApi debe ser boolean');
  if (typeof spec.internalApi !== 'boolean') push('spec.internalApi debe ser boolean');

  try {
    assertStringArray(spec.capabilities, 'spec.capabilities');
    assertStringArray(spec.environments, 'spec.environments');
  } catch (error) {
    push(error.message);
  }
  for (const environment of spec.environments ?? []) {
    if (!ENVIRONMENTS.includes(environment)) {
      push(`spec.environments contiene un entorno desconocido: ${environment}`);
    }
  }

  const dependencies = spec.dependencies ?? {};
  try {
    assertStringArray(dependencies.sync ?? [], 'spec.dependencies.sync');
    assertStringArray(dependencies.async ?? [], 'spec.dependencies.async');
  } catch (error) {
    push(error.message);
  }

  const events = spec.events ?? {};
  try {
    assertStringArray(events.publishes ?? [], 'spec.events.publishes');
    assertStringArray(events.consumes ?? [], 'spec.events.consumes');
  } catch (error) {
    push(error.message);
  }

  const slo = spec.slo ?? {};
  if (typeof slo.availability !== 'number' || slo.availability <= 0 || slo.availability > 100) {
    push('spec.slo.availability debe ser un porcentaje (0,100]');
  }
  if (!Number.isInteger(slo.latencyP95Ms) || slo.latencyP95Ms <= 0) {
    push('spec.slo.latencyP95Ms debe ser un entero positivo');
  }

  const docs = spec.docs ?? {};
  if (docs.readme !== 'README.md') {
    push('spec.docs.readme debe ser "README.md"');
  }
  if (docs.openapi !== null && typeof docs.openapi !== 'string') {
    push('spec.docs.openapi debe ser string o null');
  }

  const quality = manifest.quality?.coverage;
  if (
    !quality ||
    !Number.isInteger(quality.lines) ||
    !Number.isInteger(quality.functions) ||
    quality.lines < 0 ||
    quality.lines > 100 ||
    quality.functions < 0 ||
    quality.functions > 100
  ) {
    push('quality.coverage debe declarar lines/functions como porcentajes enteros');
  }

  return errors;
}

function loadManifests() {
  const dirs = packageDirs();
  const manifests = [];
  for (const dir of dirs) {
    const file = join(REPO_ROOT, PACKAGES_DIR, dir, SERVICE_MANIFEST_FILE);
    if (!existsSync(file)) {
      throw new ServiceCatalogError(
        `${PACKAGES_DIR}/${dir}/${SERVICE_MANIFEST_FILE}: falta el manifiesto de servicio (R-MS-9)`,
      );
    }
    const manifest = readJson(file);
    const errors = validateManifestShape(manifest, `${PACKAGES_DIR}/${dir}/${SERVICE_MANIFEST_FILE}`);
    if (errors.length > 0) {
      throw new ServiceCatalogError(errors.join('\n'));
    }
    manifests.push({ dir, file, manifest });
  }
  return manifests;
}

const loaded = loadManifests();

/** @type {ReadonlyArray<{service: string, dir: string, file: string, manifest: object}>} */
export const SERVICE_MANIFESTS = Object.freeze(loaded);

/** Lista ordenada de servicios desplegables (derivada de los manifiestos). */
export const SERVICES = Object.freeze(loaded.map((entry) => entry.dir));

/** Todos los servicios son crates Rust (R-MS-1). */
export const RUST_SERVICES = SERVICES;

/** Nombre del servicio → entrada completa del catálogo. */
export const SERVICE_BY_NAME = Object.freeze(
  Object.fromEntries(loaded.map((entry) => [entry.dir, entry.manifest])),
);

/** Nombre del servicio → puerto declarado. */
export const SERVICE_PORTS = Object.freeze(
  Object.fromEntries(loaded.map((entry) => [entry.dir, entry.manifest.spec.port])),
);

/** Nombre del servicio → perfil arquitectónico. */
export const SERVICE_PROFILES = Object.freeze(
  Object.fromEntries(loaded.map((entry) => [entry.dir, entry.manifest.spec.profile])),
);

/**
 * Matriz de cobertura que consume `.github/workflows/ci.yml`.
 * Se emite como JSON de una sola línea para `$GITHUB_OUTPUT`.
 */
export function coverageMatrix() {
  return loaded.map((entry) => ({
    service: entry.dir,
    dir: `${PACKAGES_DIR}/${entry.dir}`,
    lines: entry.manifest.quality.coverage.lines,
    functions: entry.manifest.quality.coverage.functions,
  }));
}
