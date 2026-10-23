import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compareSemver, parseTag } from './semver.mjs';

test('semver: acepta tags válidos y rechaza formatos inválidos', () => {
  assert.deepEqual(parseTag('v1.2.3'), {
    raw: 'v1.2.3',
    major: 1,
    minor: 2,
    patch: 3,
    prerelease: [],
  });
  assert.deepEqual(parseTag('v2.0.0-rc.1')?.prerelease, ['rc', '1']);

  for (const invalid of ['1.2.3', 'v1.2', 'v1.2.3.4', 'v1.2.3-', 'versión']) {
    assert.equal(parseTag(invalid), null, `debería rechazar ${invalid}`);
  }
});

test('semver: ordena versiones por major, minor y patch', () => {
  const sorted = ['v1.0.0', 'v2.0.0', 'v1.10.0', 'v1.2.0'].sort((a, b) =>
    compareSemver(parseTag(a), parseTag(b)),
  );
  assert.deepEqual(sorted, ['v1.0.0', 'v1.2.0', 'v1.10.0', 'v2.0.0']);
});

test('semver: un prerelease es menor que la versión estable', () => {
  const prerelease = parseTag('v1.0.7-rc.1');
  const stable = parseTag('v1.0.7');
  assert.equal(compareSemver(prerelease, stable), -1);
  assert.equal(compareSemver(stable, prerelease), 1);
});

test('semver: compara identificadores numéricos como números y alfanuméricos como texto', () => {
  assert.equal(compareSemver(parseTag('v1.0.7-rc.2'), parseTag('v1.0.7-rc.10')), -1);
  assert.equal(compareSemver(parseTag('v1.0.7-alpha'), parseTag('v1.0.7-beta')), -1);
  assert.equal(compareSemver(parseTag('v1.0.7-1'), parseTag('v1.0.7-alpha')), -1);
});
