/**
 * Validación de asuntos de commit contra Conventional Commits, parametrizada por
 * los tipos y ámbitos admitidos del proyecto. Es pura: se prueba con datos
 * sintéticos (mode `pure`) y se reutiliza en la suite contextual de la rama.
 */

const GENERIC_SUMMARIES = new Set([
  'wip',
  'update',
  'updates',
  'fix',
  'fixes',
  'change',
  'changes',
  'arreglos',
  'cambios',
  'varios',
  'varias',
  'cosas',
  'stuff',
  'temp',
  'tmp',
  'fixup',
  'squash',
  'mejoras',
  'ajustes',
]);

const PAST_FIRST_WORDS = new Set([
  'corregí',
  'corregi',
  'agregué',
  'agregue',
  'añadí',
  'anadí',
  'arreglé',
  'arregle',
  'cambié',
  'cambie',
  'actualicé',
  'eliminé',
  'elimine',
  'creé',
  'cree',
  'moví',
  'subí',
  'quité',
  'mejoré',
  'hice',
  'dejé',
  'puse',
  'saqué',
  'terminé',
  'revisé',
  'probé',
  'validé',
  'ajusté',
  'documenté',
  'añadido',
  'corregido',
  'eliminado',
  'movido',
  'actualizado',
  'separado',
  'extraído',
]);

/**
 * Valida un asunto de commit contra R-GC-1..4.
 * @param {string} subject Asunto del commit (`tipo(ámbito): resumen`).
 * @param {{ types: string[], scopes: string[] }} allowed Tipos y ámbitos admitidos.
 * @returns {string[]} Problemas encontrados (vacío si es conforme).
 */
export function validateCommitSubject(subject, { types, scopes }) {
  const match = /^([a-z]+)(?:\(([a-z0-9/_-]+)\))?(!)?: (.+)$/.exec(subject ?? '');
  if (!match) {
    return ['no sigue el formato `<tipo>(<ámbito>): <resumen>` (R-GC-1)'];
  }

  const [, type, scope, , summary] = match;
  const errors = [];

  if (!types.includes(type)) {
    errors.push(`tipo '${type}' no admitido (R-GC-1)`);
  }
  if (!scope) {
    errors.push('falta el ámbito (R-GC-1)');
  } else if (!scopes.includes(scope)) {
    errors.push(`ámbito '${scope}' no reconocido para este repo (R-GC-1)`);
  }

  const clean = summary.trim();
  const firstWord = clean.split(/\s+/)[0].toLowerCase();

  if (clean.length < 3) {
    errors.push('resumen demasiado corto (R-GC-2)');
  }
  if (GENERIC_SUMMARIES.has(firstWord) || GENERIC_SUMMARIES.has(clean.toLowerCase())) {
    errors.push(`resumen genérico prohibido ('${firstWord}') (R-GC-2)`);
  }
  if (/(?:ando|iendo)$/.test(firstWord)) {
    errors.push(`resumen en gerundio ('${firstWord}'); usa imperativo (R-GC-1)`);
  }
  if (PAST_FIRST_WORDS.has(firstWord)) {
    errors.push(`resumen en pasado ('${firstWord}'); usa imperativo (R-GC-1)`);
  }

  return errors;
}
