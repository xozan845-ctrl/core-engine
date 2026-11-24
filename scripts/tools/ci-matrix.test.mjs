import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';
import { SERVICES } from '../config/project.mjs';

const script = join(repoRoot, 'scripts', 'tools', 'ci-matrix.mjs');

test('ci-matrix: emite un array compatible con matrix.include', () => {
  const output = execFileSync(process.execPath, [script], { encoding: 'utf8' }).trim();
  const matrix = JSON.parse(output);
  assert.ok(Array.isArray(matrix), 'matrix.include necesita un array, no { include }');
  assert.equal(matrix.length, SERVICES.length);
  for (const entry of matrix) {
    assert.ok(SERVICES.includes(entry.service));
    assert.match(entry.dir, /^packages\/[a-z-]+$/);
    assert.equal(typeof entry.lines, 'number');
    assert.equal(typeof entry.functions, 'number');
  }
});
