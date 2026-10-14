/**
 * Lectura del árbol normativo `docs/rules`: áreas, archivos `NN-tema.md` y los
 * IDs de regla definidos en tablas `| ID | Regla |`. Genérico: recibe la raíz.
 */
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { walkMarkdown } from './fs.mjs';

const RULE_ID_ROW = /^\|\s*(?:\*\*)?((?:R-[A-Z]+|G)-\d+)(?:\*\*)?\s*\|/gm;

/** Áreas de `docs/rules` (carpetas), excluyendo `_meta`. */
export function getRuleAreas(rulesRoot) {
  return readdirSync(rulesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name !== '_meta')
    .map((entry) => entry.name)
    .sort();
}

/** Archivos de reglas `NN-tema.md` de todas las áreas. */
export function getRuleFiles(rulesRoot) {
  return getRuleAreas(rulesRoot).flatMap((area) =>
    readdirSync(join(rulesRoot, area), { withFileTypes: true })
      .filter((file) => file.isFile() && /^\d{2}-.+\.md$/.test(file.name))
      .map((file) => join(rulesRoot, area, file.name)),
  );
}

/** IDs de regla/gate definidos en tablas, en todo el árbol de reglas. */
export function collectRuleIds(rulesRoot) {
  const ids = new Set();
  for (const file of getRuleFiles(rulesRoot)) {
    const content = readFileSync(file, 'utf8');
    for (const match of content.matchAll(RULE_ID_ROW)) {
      ids.add(match[1]);
    }
  }
  return ids;
}

export { walkMarkdown };
