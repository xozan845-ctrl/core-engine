import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const script = new URL('./promotion-check.mjs', import.meta.url).pathname;

function git(cwd, args) {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim();
}

function createRepo(t) {
  const root = mkdtempSync(join(tmpdir(), 'promotion-check-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  git(root, ['init', '-q', '-b', 'main']);
  git(root, ['config', 'user.email', 'test@example.test']);
  git(root, ['config', 'user.name', 'Promotion Test']);
  writeFileSync(join(root, 'base.txt'), 'base\n');
  git(root, ['add', '-A']);
  git(root, ['commit', '-q', '-m', 'base']);

  git(root, ['branch', 'staging']);
  git(root, ['switch', '-q', '-c', 'develop']);
  writeFileSync(join(root, 'feature.txt'), 'feature\n');
  git(root, ['add', '-A']);
  git(root, ['commit', '-q', '-m', 'feature']);

  return root;
}

function run(root, args) {
  return spawnSync(process.execPath, [script, '--root', root, ...args], {
    encoding: 'utf8',
  });
}

test('promotion-check: acepta una promoción por delta con árbol idéntico', (t) => {
  const root = createRepo(t);
  git(root, ['switch', '-q', '-c', 'promote', 'staging']);
  git(root, ['checkout', 'develop', '--', 'feature.txt']);
  git(root, ['add', '-A']);
  git(root, ['commit', '-q', '-m', 'promote delta']);

  const result = run(root, [
    '--source',
    'develop',
    '--destination',
    'staging',
    '--branch',
    'promote',
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Promoción válida/);
});

test('promotion-check: falla si falta contenido del origen', (t) => {
  const root = createRepo(t);
  git(root, ['switch', '-q', '-c', 'promote', 'staging']);

  const result = run(root, [
    '--source',
    'develop',
    '--destination',
    'staging',
    '--branch',
    'promote',
  ]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /no tiene el mismo contenido/);
});

test('promotion-check: falla si la rama no nace del destino', (t) => {
  const root = createRepo(t);
  git(root, ['switch', '-q', 'staging']);
  writeFileSync(join(root, 'staging-only.txt'), 'staging\n');
  git(root, ['add', '-A']);
  git(root, ['commit', '-q', '-m', 'staging only']);
  git(root, ['switch', '-q', '-c', 'promote', 'develop']);

  const result = run(root, [
    '--source',
    'develop',
    '--destination',
    'staging',
    '--branch',
    'promote',
  ]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /no nace de 'staging'/);
});
