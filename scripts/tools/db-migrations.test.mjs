import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const script = new URL('./db-migrations.mjs', import.meta.url).pathname;

function createFixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'db-migrations-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, 'infra', 'db', 'init'), { recursive: true });
  writeFileSync(join(root, 'infra', 'db', 'init', '01_base.sql'), 'CREATE TABLE a (id int);\n');
  return {
    root,
    manifest: join(root, 'db-migrations.json'),
    migration: join(root, 'infra', 'db', 'init', '01_base.sql'),
  };
}

function run(root, manifest, ...args) {
  return spawnSync(
    process.execPath,
    [script, '--root', root, '--manifest', manifest, ...args],
    { encoding: 'utf8' },
  );
}

test('db-migrations: registra y verifica un manifiesto íntegro', (t) => {
  const { root, manifest } = createFixture(t);

  const update = run(root, manifest, '--update');
  assert.equal(update.status, 0, update.stderr);
  assert.ok(JSON.parse(readFileSync(manifest, 'utf8'))['01_base.sql']);

  const check = run(root, manifest, '--check');
  assert.equal(check.status, 0, check.stderr);
});

test('db-migrations: falla si una migración registrada se edita', (t) => {
  const { root, manifest, migration } = createFixture(t);
  run(root, manifest, '--update');
  writeFileSync(migration, 'CREATE TABLE a (id int, nombre text);\n');

  const check = run(root, manifest, '--check');
  assert.equal(check.status, 1);
  assert.match(check.stderr, /01_base\.sql: hash distinto al registrado/);
});

test('db-migrations: falla si aparece una migración sin registrar', (t) => {
  const { root, manifest } = createFixture(t);
  run(root, manifest, '--update');
  writeFileSync(join(root, 'infra', 'db', 'init', '02_nueva.sql'), 'SELECT 1;\n');

  const check = run(root, manifest, '--check');
  assert.equal(check.status, 1);
  assert.match(check.stderr, /02_nueva\.sql: migración nueva sin registrar/);
});
