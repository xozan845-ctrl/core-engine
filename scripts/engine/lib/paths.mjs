/**
 * Rutas del motor. El motor vive en `scripts/engine/lib/`, así que la raíz del
 * repositorio está tres niveles arriba. Aquí solo se resuelven ubicaciones: el
 * motor NO conoce datos del proyecto (eso vive en `scripts/config/`).
 */
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));

/** Directorio `scripts/`. */
export const scriptsRoot = resolve(here, '..', '..');

/** Raíz del repositorio (un nivel por encima de `scripts/`). */
export const repoRoot = resolve(scriptsRoot, '..');

/** Directorio de suites de reglas (`scripts/rules`). */
export const rulesDir = resolve(scriptsRoot, 'rules');

/** Resuelve una ruta relativa a la raíz del repositorio. */
export function resolveInRepo(...segments) {
  return resolve(repoRoot, ...segments);
}

/** Convierte una ruta absoluta a URL `file://` (para `import()` dinámico). */
export function toFileUrl(path) {
  return pathToFileURL(path).href;
}
