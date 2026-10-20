import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const script = new URL('./secrets-check.mjs', import.meta.url).pathname;

function git(cwd, args) {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim();
}

function createRepo(t) {
  const root = mkdtempSync(join(tmpdir(), 'secrets-check-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  git(root, ['init', '-q', '-b', 'main']);
  git(root, ['config', 'user.email', 'test@example.test']);
  git(root, ['config', 'user.name', 'Secrets Test']);
  writeFileSync(join(root, 'app.txt'), 'const foo = "bar";\n');
  git(root, ['add', '-A']);
  git(root, ['commit', '-q', '-m', 'base']);
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [script, '--root', root], { encoding: 'utf8' });
}

test('secrets-check: acepta archivos versionados sin patrones de alto riesgo', (t) => {
  const result = run(createRepo(t));
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Sin secretos de alta confianza/);
});

test('secrets-check: detecta una clave AWS versionada', (t) => {
  const root = createRepo(t);
  writeFileSync(join(root, 'config.env'), 'AWS_ACCESS_KEY_ID=AKIAABCDEFGHIJKLMNOP\n');
  git(root, ['add', '-A']);
  git(root, ['commit', '-q', '-m', 'add config']);

  const result = run(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /config\.env:1: AWS access key/);
});

test('secrets-check: ignora archivos no versionados', (t) => {
  const root = createRepo(t);
  writeFileSync(join(root, 'tmp.txt'), 'AKIAABCDEFGHIJKLMNOP\n');

  const result = run(root);
  assert.equal(result.status, 0, result.stderr);
});
