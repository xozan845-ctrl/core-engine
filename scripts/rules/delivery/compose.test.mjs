import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';
import { checkEnv } from '../../engine/lib/env.mjs';
import { GHCR_OWNER, SERVICES, WORKFLOWS_DIR } from '../../config/project.mjs';

/**
 * Reglas de entrega: los composes de stage/prod consumen las imágenes de GHCR
 * sin reconstruir (R-CD-3) y los workflows de release/promote/deploy existen y
 * cumplen su contrato (R-CD-1, R-CD-2, R-CD-8).
 */

// Excluidos de los composes de producción por decisión del repo (viven en el
// entorno de desarrollo `docker-compose.dev.yml`).
const EXCLUDED_FROM_PROD = ['commissions-service', 'market-intelligence-service'];

const read = (...segments) => readText(join(repoRoot, ...segments));
const workflowsDirectory = join(repoRoot, WORKFLOWS_DIR);

export default defineSuite({
  id: 'delivery-compose',
  area: 'cd',
  rules: ['R-CD-1', 'R-CD-2', 'R-CD-3', 'R-CD-7', 'R-CD-8', 'R-CD-12', 'G-6'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-CD-3: los composes de stage/prod consumen GHCR, sin `build:`', () => {
      for (const [rel, tag] of [
        ['docker-compose.stage.yml', 'stage'],
        ['docker-compose.yml', 'prod'],
      ]) {
        assert.ok(existsSync(join(repoRoot, rel)), `falta ${rel}`);
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
      assert.ok(existsSync(join(repoRoot, rel)), `falta ${rel}`);
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
      const files = ['ci.yml', 'release.yml', 'promote.yml', 'deploy.yml', 'e2e.yml'];
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
        release.includes('scripts/tools/release-check.mjs'),
        'release.yml debe validar tag, monotonicidad y versiones con release-check (R-CD-1/R-CD-2)',
      );
      assert.ok(
        existsSync(join(repoRoot, 'scripts', 'tools', 'release-check.mjs')),
        'falta scripts/tools/release-check.mjs',
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

      const deploy = read('.github', 'workflows', 'deploy.yml');
      assert.ok(
        deploy.includes('npm run demo'),
        'deploy.yml debe ejecutar el smoke E2E de dev (R-CD-12)',
      );
      assert.ok(
        deploy.includes('npm run smoke:remote'),
        'deploy.yml debe ejecutar el smoke remoto en stage/prod (R-CD-12)',
      );
      assert.ok(
        existsSync(join(repoRoot, 'scripts', 'tools', 'smoke-remote.mjs')),
        'falta scripts/tools/smoke-remote.mjs',
      );

      const e2e = read('.github', 'workflows', 'e2e.yml');
      assert.ok(
        e2e.includes('docker-compose.dev.yml'),
        'e2e.yml debe levantar el stack dev (G-6)',
      );
      assert.ok(
        e2e.includes('npm run demo'),
        'e2e.yml debe ejecutar el flujo E2E completo (G-6)',
      );
    });

    test('R-CD-7: compose y plantillas de entorno están alineados y sin secretos', () => {
      const problems = checkEnv(repoRoot);
      assert.deepEqual(
        problems,
        [],
        `Entornos inconsistentes (R-CD-7):\n${problems.join('\n')}`,
      );
    });

    test('R-CD-7: hay plantilla de entorno por entorno y `.env.*` está ignorado', () => {
      for (const f of ['.env.example', '.env.stage.example', '.env.production.example']) {
        assert.ok(existsSync(join(repoRoot, f)), `falta la plantilla ${f}`);
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
  },
});
