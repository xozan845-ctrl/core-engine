import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compareCoverage } from './coverage.mjs';

const baseline = {
  shared: { lines: 50, branches: 40, functions: 45, statements: 48 },
  orders: { lines: 30, branches: 20, functions: 25, statements: 28 },
};

test('coverage: acepta valores iguales o superiores al baseline', () => {
  const current = {
    shared: { lines: 50, branches: 41, functions: 45, statements: 49 },
    orders: { lines: 30, branches: 20, functions: 25, statements: 28 },
  };
  assert.deepEqual(compareCoverage(current, baseline), []);
});

test('coverage: detecta cada métrica que baja', () => {
  const current = {
    shared: { lines: 49.5, branches: 39, functions: 44, statements: 48 },
    orders: { lines: 30, branches: 20, functions: 25, statements: 28 },
  };
  const problems = compareCoverage(current, baseline);
  assert.equal(problems.length, 3);
  assert.ok(problems.some((p) => p.includes('shared: lines 49.5% < baseline 50%')));
  assert.ok(problems.some((p) => p.includes('shared: branches 39% < baseline 40%')));
  assert.ok(problems.some((p) => p.includes('shared: functions 44% < baseline 45%')));
});

test('coverage: falla si falta un workspace en el reporte o en el baseline', () => {
  const current = {
    shared: { lines: 50, branches: 40, functions: 45, statements: 48 },
    nuevo: { lines: 10, branches: 10, functions: 10, statements: 10 },
  };
  const problems = compareCoverage(current, baseline);
  assert.ok(problems.some((p) => p.includes('orders: sin reporte de cobertura')));
  assert.ok(problems.some((p) => p.includes('nuevo: sin baseline')));
});

test('coverage: falla si el baseline no declara una métrica', () => {
  const problems = compareCoverage(
    { shared: { lines: 50, branches: 40, functions: 45, statements: 48 } },
    { shared: { lines: 50 } },
  );
  assert.ok(problems.some((p) => p.includes("baseline sin la métrica 'branches'")));
});
