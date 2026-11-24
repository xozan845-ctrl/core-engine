import test from 'node:test';
import assert from 'node:assert/strict';
import { compareBaselines } from './baseline-guard.mjs';
import { RUST_SERVICES } from '../config/project.mjs';

function documents(overrides = {}) {
  return {
    coverage: { shared: { lines: 50, branches: 40, functions: 45, statements: 48 } },
    specs: ['packages/a/src/a.service.ts'],
    rls: ['catalog.outbox'],
    migrations: { '01_base.sql': 'hash-a' },
    moduleSize: {
      limits: { lines: 800, publicFns: 20, sqlTables: 8 },
      debt: {
        'packages/a/src/a.rs': { lines: 1600, publicFns: 10, sqlTables: 2 },
      },
    },
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

test('baseline-guard: rechaza crecer o añadir deuda de tamaño de módulos (R-MS-7)', () => {
  const grown = compareBaselines(
    documents({
      moduleSize: {
        limits: { lines: 800, publicFns: 20, sqlTables: 8 },
        debt: {
          'packages/a/src/a.rs': { lines: 1700, publicFns: 10, sqlTables: 2 },
          'packages/b/src/b.rs': { lines: 1550, publicFns: 10, sqlTables: 2 },
        },
      },
    }),
    documents(),
  );
  assert.match(grown.join('\n'), /moduleSize: packages\/a\/src\/a\.rs\.lines creció/);
  assert.match(grown.join('\n'), /moduleSize: nueva deuda\/excepción no permitida/);
});

test('baseline-guard: rechaza relajar los presupuestos de R-MS-7', () => {
  const problems = compareBaselines(
    documents({
      moduleSize: {
        limits: { lines: 1500, publicFns: 20, sqlTables: 8 },
        debt: documents().moduleSize.debt,
      },
    }),
    documents(),
  );
  assert.match(problems.join('\n'), /moduleSize\.limits\.lines: se relajó/);
});

test('baseline-guard: permite dividir un módulo y retirarlo del baseline', () => {
  const problems = compareBaselines(
    documents({
      moduleSize: {
        limits: documents().moduleSize.limits,
        debt: {},
      },
    }),
    documents(),
  );
  assert.deepEqual(problems, []);
});

test('baseline-guard: permite retirar un workspace migrado a Rust', () => {
  const metrics = { lines: 90, branches: 80, functions: 85, statements: 88 };
  const problems = compareBaselines(
    documents(),
    documents({
      coverage: {
        ...documents().coverage,
        [RUST_SERVICES[0]]: metrics,
      },
    }),
  );
  assert.deepEqual(problems, []);
});
