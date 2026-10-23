import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { SERVICES } from '../config/project.mjs';

const script = new URL('./release-check.mjs', import.meta.url).pathname;
const workspaces = ['shared', ...SERVICES];

function git(cwd, args) {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim();
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function writeVersion(root, version) {
  writeJson(join(root, 'package.json'), {
    name: 'fixture',
    version,
    private: true,
    workspaces: ['packages/*'],
  });
  writeJson(join(root, 'package-lock.json'), {
    name: 'fixture',
    version,
    lockfileVersion: 3,
    packages: { '': { name: 'fixture', version } },
  });
  for (const workspace of workspaces) {
    const dir = join(root, 'packages', workspace);
    mkdirSync(dir, { recursive: true });
    writeJson(join(dir, 'package.json'), {
      name: `@core/${workspace}`,
      version,
      private: true,
    });
  }
}

function createFixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'release-check-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  git(root, ['init', '-q', '-b', 'main']);
  git(root, ['config', 'user.email', 'test@example.test']);
  git(root, ['config', 'user.name', 'Release Check Test']);

  writeVersion(root, '1.0.6');
  git(root, ['add', '-A']);
  git(root, ['commit', '-q', '-m', 'release 1.0.6']);
  git(root, ['tag', '-a', 'v1.0.6', '-m', 'Release v1.0.6']);
  const first = git(root, ['rev-parse', 'HEAD']);

  writeVersion(root, '1.0.7');
  git(root, ['add', '-A']);
  git(root, ['commit', '-q', '-m', 'release 1.0.7']);
  git(root, ['tag', '-a', 'v1.0.7', '-m', 'Release v1.0.7']);
  const second = git(root, ['rev-parse', 'HEAD']);

  return { root, first, second };
}

function runCheck(root, args) {
  return spawnSync(process.execPath, [script, '--root', root, ...args], {
    encoding: 'utf8',
  });
}

test('release-check: acepta un tag nuevo, anotado y con versiones alineadas', (t) => {
  const { root } = createFixture(t);
  const result = runCheck(root, ['--tag', 'v1.0.7', '--main-ref', 'HEAD']);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Release 'v1\.0\.7' válida/);
  assert.match(result.stdout, /versiones: 1\.0\.7/);
});

test('release-check: exime de versión a los tags históricos', (t) => {
  const { root, first } = createFixture(t);
  const result = runCheck(root, ['--tag', 'v1.0.6', '--main-ref', first]);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /versiones: omitidas/);
});

test('release-check: rechaza un tag que no supera al último release', (t) => {
  const { root } = createFixture(t);
  const result = runCheck(root, ['--tag', 'v1.0.6', '--main-ref', 'HEAD']);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /no es mayor que el último release 'v1\.0\.7'/);
});

test('release-check: rechaza un tag lightweight', (t) => {
  const { root } = createFixture(t);
  git(root, ['tag', 'v1.0.8']);
  const result = runCheck(root, ['--tag', 'v1.0.8', '--main-ref', 'HEAD']);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /debe ser anotado/);
});

test('release-check: rechaza versiones desalineadas desde VERSION_MATCH_FROM', (t) => {
  const { root } = createFixture(t);
  git(root, ['tag', '-a', 'v1.0.9', '-m', 'Release v1.0.9']);
  const result = runCheck(root, ['--tag', 'v1.0.9', '--main-ref', 'HEAD']);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /Las versiones no coinciden con v1\.0\.9/);
  assert.match(result.stderr, /package\.json: version '1\.0\.7' != '1\.0\.9'/);
});
