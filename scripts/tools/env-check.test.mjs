import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const script = new URL('./env-check.mjs', import.meta.url).pathname;

function createFixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'env-check-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  for (const name of ['.env.example', '.env.stage.example', '.env.production.example']) {
    writeFileSync(join(root, name), 'FOO=bar\nPOSTGRES_PASSWORD=change-me\n');
  }
  writeFileSync(
    join(root, 'docker-compose.yml'),
    'services:\n  db:\n    environment:\n      FOO: ${FOO}\n      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}\n      ENV_FILE: ${ENV_FILE}\n',
  );
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [script, '--root', root], { encoding: 'utf8' });
}

test('env-check: acepta compose alineado y ENV_FILE externo', (t) => {
  const result = run(createFixture(t));
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /alineados/);
});

test('env-check: falla si una plantilla no declara una variable del compose', (t) => {
  const root = createFixture(t);
  writeFileSync(join(root, '.env.stage.example'), 'POSTGRES_PASSWORD=change-me\n');
  const result = run(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /\.env\.stage\.example: falta FOO/);
});

test('env-check: falla si un secreto queda hardcodeado', (t) => {
  const root = createFixture(t);
  writeFileSync(
    join(root, 'docker-compose.yml'),
    'services:\n  db:\n    environment:\n      POSTGRES_PASSWORD: supersecreto\n',
  );
  const result = run(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /POSTGRES_PASSWORD: valor hardcodeado/);
});
