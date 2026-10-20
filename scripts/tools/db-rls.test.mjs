import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const script = new URL('./db-rls.mjs', import.meta.url).pathname;

function createFixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'db-rls-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  mkdirSync(join(root, 'infra', 'db', 'init'), { recursive: true });
  mkdirSync(join(root, 'infra', 'db', 'supabase'), { recursive: true });
  writeFileSync(
    join(root, 'infra', 'db', 'init', '01_base.sql'),
    [
      'CREATE TABLE IF NOT EXISTS a.ok (id int);',
      'CREATE TABLE IF NOT EXISTS a.legacy (id int);',
      '',
    ].join('\n'),
  );
  writeFileSync(
    join(root, 'infra', 'db', 'supabase', '99_rls.sql'),
    [
      'ALTER TABLE a.ok ENABLE ROW LEVEL SECURITY;',
      'CREATE POLICY ok_all ON a.ok FOR ALL USING (true);',
      '',
    ].join('\n'),
  );

  const baseline = join(root, 'rls-baseline.json');
  writeFileSync(baseline, `${JSON.stringify(['a.legacy'], null, 2)}\n`);
  return { root, baseline };
}

function run(root, baseline, ...args) {
  return spawnSync(
    process.execPath,
    [script, '--root', root, '--baseline', baseline, ...args],
    { encoding: 'utf8' },
  );
}

test('db-rls: acepta la deuda explícita y las tablas cubiertas', (t) => {
  const { root, baseline } = createFixture(t);
  const result = run(root, baseline);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /1 tablas en el baseline RLS/);
});

test('db-rls: falla si aparece una tabla nueva sin RLS', (t) => {
  const { root, baseline } = createFixture(t);
  writeFileSync(
    join(root, 'infra', 'db', 'init', '02_nueva.sql'),
    'CREATE TABLE IF NOT EXISTS a.nueva (id int);\n',
  );

  const result = run(root, baseline);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /a\.nueva: sin RLS o sin policy/);
});

test('db-rls: falla si el baseline queda obsoleto', (t) => {
  const { root, baseline } = createFixture(t);
  writeFileSync(
    join(root, 'infra', 'db', 'supabase', '99_rls.sql'),
    [
      'ALTER TABLE a.ok ENABLE ROW LEVEL SECURITY;',
      'CREATE POLICY ok_all ON a.ok FOR ALL USING (true);',
      'ALTER TABLE a.legacy ENABLE ROW LEVEL SECURITY;',
      'CREATE POLICY legacy_all ON a.legacy FOR ALL USING (true);',
      '',
    ].join('\n'),
  );

  const result = run(root, baseline);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /a\.legacy: ya tiene RLS y policy; elimina la excepción/);
});

test('db-rls: --update regenera el baseline con la deuda actual', (t) => {
  const { root, baseline } = createFixture(t);
  writeFileSync(
    join(root, 'infra', 'db', 'init', '02_nueva.sql'),
    'CREATE TABLE IF NOT EXISTS a.nueva (id int);\n',
  );

  const update = run(root, baseline, '--update');
  assert.equal(update.status, 0, update.stderr);
  assert.deepEqual(JSON.parse(readFileSync(baseline, 'utf8')), ['a.legacy', 'a.nueva']);
});
