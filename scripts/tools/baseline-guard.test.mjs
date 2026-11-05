import test from 'node:test';
import assert from 'node:assert/strict';
import { compareBaselines } from './baseline-guard.mjs';

function documents(overrides = {}) {
  return {
    coverage: { shared: { lines: 50, branches: 40, functions: 45, statements: 48 } },
    specs: ['packages/a/src/a.service.ts'],
    rls: ['catalog.outbox'],
    migrations: { '01_base.sql': 'hash-a' },
    breaking: { allow: ['GET /old: removed'] },
    ...overrides,
  };
}

test('baseline-guard: acepta mejoras y migraciones nuevas', () => {
  const problems = compareBaselines(
    documents({
      coverage: { shared: { lines: 55, branches: 40, functions: 50, statements: 49 } },
      specs: [],
      rls: [],
      migrations: { '01_base.sql': 'hash-a', '02_new.sql': 'hash-b' },
      breaking: { allow: ['GET /old: removed'] },
    }),
    documents(),
  );
  assert.deepEqual(problems, []);
});

test('baseline-guard: rechaza bajar cobertura', () => {
  const problems = compareBaselines(
    documents({ coverage: { shared: { lines: 49, branches: 40, functions: 45, statements: 48 } } }),
    documents(),
  );
  assert.match(problems.join('\n'), /coverage\.shared\.lines/);
});

test('baseline-guard: rechaza aumentar deuda de specs o RLS', () => {
  const problems = compareBaselines(
    documents({ specs: ['packages/a/src/a.service.ts', 'packages/b/src/b.service.ts'], rls: ['catalog.outbox', 'orders.outbox'] }),
    documents(),
  );
  assert.match(problems.join('\n'), /specs: nueva deuda/);
  assert.match(problems.join('\n'), /rls: nueva deuda/);
});

test('baseline-guard: rechaza editar o borrar migraciones existentes', () => {
  const problems = compareBaselines(
    documents({ migrations: {} }),
    documents(),
  );
  assert.match(problems.join('\n'), /migrations: se eliminó/);

  const altered = compareBaselines(
    documents({ migrations: { '01_base.sql': 'hash-b' } }),
    documents(),
  );
  assert.match(altered.join('\n'), /migrations: hash alterado/);
});

test('baseline-guard: rechaza nuevas excepciones de contrato', () => {
  const problems = compareBaselines(
    documents({ breaking: { allow: ['GET /old: removed', 'POST /new: removed'] } }),
    documents(),
  );
  assert.match(problems.join('\n'), /breaking: nueva deuda\/excepción/);
});
