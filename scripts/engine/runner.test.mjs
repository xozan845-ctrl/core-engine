import test from 'node:test';
import assert from 'node:assert/strict';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runRules, validateManifest } from './runner.mjs';
import { scriptsRoot } from './lib/paths.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const failingRulesDir = resolve(here, 'fixtures', 'failing-rules');

test('runRules devuelve error cuando una assertion de una suite falla', async () => {
  const code = await runRules({
    rulesDir: failingRulesDir,
    requiredSuites: [
      {
        id: 'intentional-failure',
        file: 'engine/fixtures/failing-rules/intentional-failure.test.mjs',
      },
    ],
    silent: true,
  });

  assert.equal(code, 1);
});

test('validateManifest rechaza suites que no están declaradas', () => {
  assert.throws(
    () =>
      validateManifest(
        [{ id: 'extra', file: resolve(scriptsRoot, 'rules/extra.test.mjs') }],
        [],
      ),
    /fuera del manifiesto/,
  );
});

test('validateManifest exige que el id apunte al archivo declarado', () => {
  assert.throws(
    () =>
      validateManifest(
        [{ id: 'demo', file: resolve(scriptsRoot, 'rules/real.test.mjs') }],
        [{ id: 'demo', file: 'rules/esperado.test.mjs' }],
      ),
    /no coincide con el manifiesto/,
  );
});
