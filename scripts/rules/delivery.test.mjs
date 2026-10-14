import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { GHCR_OWNER, SERVICES } from '../project.config.mjs';
import { repositoryRoot, workflowsDirectory } from './helpers/repo.mjs';

/**
 * Reglas de entrega: los composes de stage/prod consumen las imágenes de GHCR
 * sin reconstruir (R-CD-3) y los workflows de release/promote/deploy existen y
 * cumplen su contrato (R-CD-1, R-CD-2, R-CD-8).
 */

// Excluidos de los composes de producción por decisión del repo (viven en el
// entorno de desarrollo `docker-compose.dev.yml`).
const EXCLUDED_FROM_PROD = ['commissions-service', 'market-intelligence-service'];

const read = (...segments) => readFileSync(join(repositoryRoot, ...segments), 'utf8');

test('R-CD-3: los composes de stage/prod consumen GHCR, sin `build:`', () => {
  for (const [rel, tag] of [
    ['docker-compose.stage.yml', 'stage'],
    ['docker-compose.yml', 'prod'],
  ]) {
    assert.ok(existsSync(join(repositoryRoot, rel)), `falta ${rel}`);
    const content = read(rel);

    assert.ok(
      !/^\s*build:/m.test(content),
      `${rel} no debe contener \`build:\` (R-CD-3: se consume la imagen publicada)`,
    );

    for (const svc of SERVICES.filter((s) => !EXCLUDED_FROM_PROD.includes(s))) {
      const image = `image: ghcr.io/${GHCR_OWNER}/core-engine-${svc}:${tag}`;
      assert.ok(content.includes(image), `${rel}: falta \`${image}\``);
      assert.ok(
        content.includes('pull_policy: always'),
        `${rel}: falta \`pull_policy: always\` para forzar la imagen promovida`,
      );
    }
  }
});

test('dev construye local: `docker-compose.dev.yml` define los 10 servicios con `build:`', () => {
  const rel = 'docker-compose.dev.yml';
  assert.ok(existsSync(join(repositoryRoot, rel)), `falta ${rel}`);
  const content = read(rel);

  assert.ok(
    /^\s*build:/m.test(content),
    `${rel} debe construir local (\`build:\`), no consumir imágenes de GHCR`,
  );
  for (const svc of SERVICES) {
    assert.ok(
      content.includes(`SERVICIO: ${svc}`),
      `${rel}: falta el servicio \`${svc}\` (dev es el único entorno con los 10)`,
    );
  }
});

test('R-CD-1/R-CD-3/R-CD-8: existen los workflows de entrega con su contrato', () => {
  const files = ['ci.yml', 'release.yml', 'promote.yml', 'deploy.yml'];
  const missing = files.filter((f) => !existsSync(join(workflowsDirectory, f)));
  assert.deepEqual(missing, [], `Faltan workflows (R-CD-1): ${missing.join(', ')}`);

  const release = read('.github', 'workflows', 'release.yml');
  assert.ok(
    release.includes("'v*.*.*'"),
    'release.yml debe dispararse con tags v*.*.* (R-CD-1)',
  );
  assert.ok(
    /merge-base --is-ancestor/.test(release),
    'release.yml debe verificar procedencia en main (R-CD-2)',
  );
  assert.ok(
    /dry_run/.test(release),
    'release.yml debe ofrecer dry_run (R-CD-8)',
  );

  const promote = read('.github', 'workflows', 'promote.yml');
  assert.ok(
    /RepoDigests|@sha256/.test(promote),
    'promote.yml debe promover por digest (R-CD-3)',
  );
});

test('R-CD-7: hay plantilla de entorno por entorno y `.env.*` está ignorado', () => {
  for (const f of ['.env.example', '.env.stage.example', '.env.production.example']) {
    assert.ok(existsSync(join(repositoryRoot, f)), `falta la plantilla ${f}`);
  }

  const gi = read('.gitignore');
  assert.match(
    gi,
    /^\.env\.\*$/m,
    '.gitignore debe ignorar `.env.*` (los secretos por entorno nunca se versionan)',
  );
  assert.match(
    gi,
    /^!\.env\*\.example$/m,
    '.gitignore debe re-incluir las plantillas `*.example`',
  );
});
