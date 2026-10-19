import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const script = new URL('./version-bump.mjs', import.meta.url).pathname;

function createFixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'version-bump-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  writeFileSync(
    join(root, 'package.json'),
    `${JSON.stringify({ name: 'fixture', version: '1.0.0', private: true, workspaces: ['packages/*'] }, null, 2)}\n`,
  );
  mkdirSync(join(root, 'packages', 'a'), { recursive: true });
  writeFileSync(
    join(root, 'packages', 'a', 'package.json'),
    `${JSON.stringify({ name: '@core/a', version: '1.0.0', private: true }, null, 2)}\n`,
  );

  return root;
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

function run(root, args) {
  return spawnSync(process.execPath, [script, '--root', root, ...args], {
    encoding: 'utf8',
  });
}

test('version-bump: alinea root, workspaces y lockfile', (t) => {
  const root = createFixture(t);
  const result = run(root, ['1.0.7']);

  assert.equal(result.status, 0, result.stderr);
  assert.equal(readJson(join(root, 'package.json')).version, '1.0.7');
  assert.equal(readJson(join(root, 'packages', 'a', 'package.json')).version, '1.0.7');
  assert.equal(readJson(join(root, 'package-lock.json')).version, '1.0.7');
});

test('version-bump: rechaza una versión inválida', (t) => {
  const root = createFixture(t);
  const result = run(root, ['1.0']);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /no es una versión semver válida/);
});

test('version-bump: --dry-run no modifica archivos', (t) => {
  const root = createFixture(t);
  const result = run(root, ['1.0.7', '--dry-run']);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /\[dry-run\]/);
  assert.equal(readJson(join(root, 'package.json')).version, '1.0.0');
});
