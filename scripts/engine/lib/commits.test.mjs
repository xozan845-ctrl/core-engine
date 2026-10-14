import test from 'node:test';
import assert from 'node:assert/strict';
import { validateCommitSubject } from './commits.mjs';

// Autotest del motor: la utilidad pura se prueba con datos sintéticos.

const allowed = { types: ['feat', 'fix'], scopes: ['core', 'api'] };

test('validateCommitSubject acepta un asunto conforme', () => {
  assert.deepEqual(validateCommitSubject('feat(core): añadir utilidad', allowed), []);
});

test('validateCommitSubject rechaza un tipo no admitido', () => {
  assert.match(
    validateCommitSubject('chore(core): algo', allowed).join(' '),
    /tipo 'chore' no admitido/,
  );
});

test('validateCommitSubject rechaza un ámbito no reconocido', () => {
  assert.match(
    validateCommitSubject('feat(zzz): algo', allowed).join(' '),
    /no reconocido/,
  );
});

test('validateCommitSubject rechaza gerundio y resumen genérico', () => {
  assert.match(
    validateCommitSubject('feat(core): agregando algo', allowed).join(' '),
    /gerundio/,
  );
  assert.match(validateCommitSubject('fix(api): wip', allowed).join(' '), /genérico/);
});
