import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const compareScript = new URL('./coverage-compare.mjs', import.meta.url).pathname;
const baselineScript = new URL('./coverage-baseline.mjs', import.meta.url).pathname;

function writeReport(root, workspace, pct) {
  const dir = join(root, 'packages', workspace, 'coverage');
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, 'coverage-summary.json'),
    `${JSON.stringify({
      total: {
        lines: { pct },
        branches: { pct },
        functions: { pct },
        statements: { pct },
      },
    })}\n`,
  );
}

function createFixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'coverage-compare-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const baselinePath = join(root, 'coverage-baseline.json');
  writeFileSync(
    baselinePath,
    `${JSON.stringify(
      {
        shared: { lines: 50, branches: 40, functions: 45, statements: 48 },
      },
      null,
      2,
    )}\n`,
  );

  writeReport(root, 'shared', 50);
  return { root, baselinePath };
}

function run(script, root, baseline, ...args) {
  return spawnSync(
    process.execPath,
    [script, '--root', root, '--baseline', baseline, ...args],
    { encoding: 'utf8' },
  );
}

test('coverage-compare: pasa cuando la cobertura no baja', (t) => {
  const { root, baselinePath } = createFixture(t);
  const result = run(compareScript, root, baselinePath);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Cobertura igual o superior al baseline/);
});

test('coverage-compare: falla cuando una métrica baja', (t) => {
  const { root, baselinePath } = createFixture(t);
  writeReport(root, 'shared', 49);

  const result = run(compareScript, root, baselinePath);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /shared: lines 49% < baseline 50%/);
});

test('coverage-baseline: escribe el baseline desde los reportes', (t) => {
  const { root, baselinePath } = createFixture(t);
  writeReport(root, 'shared', 55);

  const result = run(baselineScript, root, baselinePath);
  assert.equal(result.status, 0, result.stderr);

  const baseline = JSON.parse(readFileSync(baselinePath, 'utf8'));
  assert.equal(baseline.shared.lines, 55);
});
