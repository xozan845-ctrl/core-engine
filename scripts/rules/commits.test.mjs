import test from 'node:test';
import assert from 'node:assert/strict';
import {
  ALLOWED_COMMIT_SCOPES,
  ALLOWED_COMMIT_TYPES,
  MAIN_BRANCH,
} from '../project.config.mjs';
import { getBranchContext, getCommitsAgainstBase } from './helpers/repo.mjs';

/**
 * Regla contextual R-GC-1..4: los commits introducidos por el PR deben seguir
 * Conventional Commits **válidos para este repo**: tipo admitido, ámbito real,
 * y resumen en imperativo (no gerundio ni pasado) y no genérico.
 *
 * `validateCommitSubject` es pura y se prueba con datos sintéticos.
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
 * @returns {string[]} Lista de problemas (vacía si es conforme).
 */
function validateCommitSubject(subject) {
  const match = /^([a-z]+)(?:\(([a-z0-9/_-]+)\))?(!)?: (.+)$/.exec(subject ?? '');
  if (!match) {
    return ['no sigue el formato `<tipo>(<ámbito>): <resumen>` (R-GC-1)'];
  }

  const [, type, scope, , summary] = match;
  const errors = [];

  if (!ALLOWED_COMMIT_TYPES.includes(type)) {
    errors.push(`tipo '${type}' no admitido (R-GC-1)`);
  }
  if (!scope) {
    errors.push('falta el ámbito (R-GC-1)');
  } else if (!ALLOWED_COMMIT_SCOPES.includes(scope)) {
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

test('commits (unit): acepta asuntos conformes', () => {
  for (const subject of [
    'ci(rules): validar los commits del PR (R-GC-1)',
    'feat(orders): añadir transición de estado (R-DI-3)',
    'fix(gateway): rechazar el header sin token',
    'docs(rules): corregir enlaces internos',
    'refactor(shared): extraer el cliente de eventos',
    'build(deps): subir NestJS a la última minor',
  ]) {
    assert.deepEqual(validateCommitSubject(subject), [], `debería aceptar: ${subject}`);
  }
});

test('commits (unit): rechaza formato, tipo y ámbito inválidos', () => {
  assert.match(validateCommitSubject('wip')[0], /formato/);
  assert.match(validateCommitSubject('fix')[0], /formato/);
  assert.match(
    validateCommitSubject('feat: sin ámbito').join(' '),
    /falta el ámbito/,
  );
  assert.match(
    validateCommitSubject('feat(zzz): algo').join(' '),
    /ámbito 'zzz' no reconocido/,
  );
});

test('commits (unit): rechaza gerundio, pasado y resumen genérico', () => {
  assert.match(
    validateCommitSubject('feat(orders): agregando endpoint').join(' '),
    /gerundio/,
  );
  assert.match(
    validateCommitSubject('fix(gateway): corregí el azp').join(' '),
    /pasado/,
  );
  assert.match(
    validateCommitSubject('feat(orders): wip').join(' '),
    /genérico/,
  );
});

test('R-GC-1..4: los commits introducidos por el PR cumplen el convenio del repo', () => {
  const { currentBranch, baseBranch } = getBranchContext();
  if (currentBranch === MAIN_BRANCH || currentBranch === `origin/${MAIN_BRANCH}`) {
    return; // en main no hay commits que evaluar
  }

  const commits = getCommitsAgainstBase(baseBranch);
  if (commits.length === 0) return;

  const problems = [];
  for (const { subject } of commits) {
    const errors = validateCommitSubject(subject);
    if (errors.length > 0) {
      problems.push(`- "${subject}"\n    ${errors.join('\n    ')}`);
    }
  }

  assert.deepEqual(
    problems,
    [],
    `Commits no conformes a Conventional Commits (R-GC-1..4):\n${problems.join('\n')}`,
  );
});
