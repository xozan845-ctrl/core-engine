import test from 'node:test';
import assert from 'node:assert/strict';
import { parseGitLog } from './git.mjs';

// Autotest del motor: el parser de `git log` es puro.

test('parseGitLog separa sujeto y cuerpo por NUL y commits por RS', () => {
  const raw = 'feat(core): uno\u0000cuerpo uno\u001e\nfix(api): dos\u0000\u001e';
  assert.deepEqual(parseGitLog(raw), [
    { subject: 'feat(core): uno', body: 'cuerpo uno' },
    { subject: 'fix(api): dos', body: '' },
  ]);
});

test('parseGitLog devuelve [] para entrada vacía', () => {
  assert.deepEqual(parseGitLog(''), []);
  assert.deepEqual(parseGitLog(undefined), []);
});
