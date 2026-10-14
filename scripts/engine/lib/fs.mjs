/**
 * Utilidades de sistema de archivos del motor. Genéricas: no conocen la
 * estructura del proyecto, solo recorren y leen.
 */
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const IGNORED_DIRS = new Set([
  'node_modules',
  '.git',
  '.agents',
  'dist',
  'coverage',
]);

/** Recorre recursivamente los archivos que cumplen `predicate`, saltando ruido. */
export function walkFiles(directory, predicate = () => true) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (entry.isDirectory()) {
      return IGNORED_DIRS.has(entry.name)
        ? []
        : walkFiles(join(directory, entry.name), predicate);
    }
    return entry.isFile() && predicate(entry.name)
      ? [join(directory, entry.name)]
      : [];
  });
}

/** Recorre recursivamente todos los `.md` de un directorio. */
export const walkMarkdown = (directory) =>
  walkFiles(directory, (name) => name.endsWith('.md'));

export function readText(path) {
  return readFileSync(path, 'utf8');
}

export function readJson(path) {
  return JSON.parse(readText(path));
}
