import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { readJson, readText } from '../../engine/lib/fs.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import {
  COVERAGE_THRESHOLDS,
} from '../../config/coverage.mjs';
import {
  PACKAGES_DIR,
  RUST_SERVICES,
  SERVICES,
  WORKFLOWS_DIR,
} from '../../config/project.mjs';

const packagesRoot = join(repoRoot, PACKAGES_DIR);
const rootPackage = readJson(join(repoRoot, 'package.json'));
const ciWorkflowPath = join(repoRoot, WORKFLOWS_DIR, 'ci.yml');
const coverageBaseline = readJson(
  join(repoRoot, 'scripts', 'config', 'coverage-baseline.json'),
);

function sorted(values) {
  return [...values].sort((a, b) => a.localeCompare(b));
}

function deployedWorkspaceDirs() {
  return readdirSync(packagesRoot, { withFileTypes: true })
    .filter(
      (entry) =>
        entry.isDirectory() &&
        existsSync(join(packagesRoot, entry.name, 'Dockerfile')),
    )
    .map((entry) => entry.name);
}

function rustMatrix() {
  const workflow = readText(ciWorkflowPath);
  const start = workflow.indexOf('  rust:');
  const end = workflow.indexOf('  build:', start);
  const section = start >= 0 && end > start ? workflow.slice(start, end) : '';
  return [...section.matchAll(/^\s+- service: ([a-z-]+)$/gm)].map((match) => match[1]);
}

function ciMatrixScript() {
  return readText(join(repoRoot, 'scripts', 'tools', 'ci-matrix.mjs'));
}

export default defineSuite({
  id: 'microservices-ci-contract',
  area: 'test',
  rules: [
    'R-MS-1',
    'R-QA-1',
    'R-COV-1',
    'R-COV-4',
    'R-CI-5',
    'R-I-12',
    'R-DS-1',
    'R-C-7',
    'R-MT-1',
    'R-MT-2',
    'R-FL-3',
    'R-RB-4',
  ],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-MS-1: el inventario declara exactamente los servicios desplegables', () => {
      assert.deepEqual(
        sorted(SERVICES),
        sorted(deployedWorkspaceDirs()),
        'SERVICES debe coincidir con los directorios packages/* que tienen Dockerfile',
      );
      assert.equal(new Set(SERVICES).size, SERVICES.length, 'SERVICES no admite duplicados');
      assert.deepEqual(
        sorted(RUST_SERVICES),
        sorted(SERVICES),
        'todos los servicios deben estar en RUST_SERVICES',
      );
    });

    test('R-MS-1: cada servicio es un crate Rust sin artefactos Node', () => {
      const problems = [];
      for (const service of SERVICES) {
        const dir = join(packagesRoot, service);
        if (!existsSync(join(dir, 'Cargo.toml'))) problems.push(`${service}: falta Cargo.toml`);
        if (!existsSync(join(dir, 'Cargo.lock'))) problems.push(`${service}: falta Cargo.lock`);
        if (!existsSync(join(dir, 'src', 'main.rs'))) problems.push(`${service}: falta src/main.rs`);
        for (const nodeArtifact of ['package.json', 'tsconfig.json', 'vitest.config.ts']) {
          if (existsSync(join(dir, nodeArtifact))) {
            problems.push(`${service}: queda ${nodeArtifact}`);
          }
        }
        const dockerfile = readText(join(dir, 'Dockerfile'));
        if (/^FROM\s+node:/m.test(dockerfile)) {
          problems.push(`${service}: el Dockerfile usa Node`);
        }
        if (!/^EXPOSE\s+\d+$/m.test(dockerfile)) {
          problems.push(`${service}: el Dockerfile no declara EXPOSE`);
        }
      }
      assert.deepEqual(problems, [], problems.join('\n'));
    });

    test('R-CI-5/R-AF-3: la matriz Rust de CI se deriva de los manifiestos', () => {
      const workflow = readText(ciWorkflowPath);
      assert.deepEqual(
        rustMatrix(),
        [],
        'la matriz no debe volver a hardcodear servicios en ci.yml',
      );
      assert.match(
        workflow,
        /^ {2}matrix:$/m,
        'falta el job matrix que emite la matriz',
      );
      assert.match(
        workflow,
        /include: \$\{\{ fromJSON\(needs\.matrix\.outputs\.include\) \}\}/,
        'rust/docker-build deben consumir la matriz emitida',
      );
      assert.match(
        workflow,
        /node scripts\/tools\/ci-matrix\.mjs/,
        'el job matrix debe ejecutar ci-matrix.mjs',
      );
      assert.match(
        ciMatrixScript(),
        /coverageMatrix\(\)/,
        'ci-matrix.mjs debe usar coverageMatrix() de los manifiestos',
      );
      assert.doesNotMatch(workflow, /^ {2}test:$/m, 'no debe quedar el job Node de tests');
      assert.doesNotMatch(workflow, /npm run build$/m, 'no debe quedar build Node');
      assert.doesNotMatch(workflow, /npm test$/m, 'no debe quedar npm test Node');
    });

    test('R-COV-1/R-COV-4: la cobertura Node retirada no conserva umbrales', () => {
      assert.deepEqual(COVERAGE_THRESHOLDS, {}, 'coverage.mjs debe quedar vacío');
      assert.deepEqual(coverageBaseline, {}, 'el baseline Node debe quedar vacío');
      assert.ok(
        rootPackage.scripts['test:integration'] === 'node scripts/tools/integration-runner.mjs',
        'falta el runner de integración',
      );
      assert.ok(rootPackage.scripts['rules:check'], 'falta rules:check');
      assert.ok(rootPackage.scripts['test:engine'], 'falta test:engine');
      assert.ok(rootPackage.scripts['test:smoke'], 'falta test:smoke');
    });

    test('R-I-12: la integración usa PostgreSQL efímero en CI', () => {
      const workflow = readText(ciWorkflowPath);
      assert.match(workflow, /^ {2}integration-catalog:$/m);
      assert.match(workflow, /^ {8}image: postgres:16-alpine$/m);
      assert.match(
        workflow,
        /^ {6}TEST_DATABASE_URL: postgres:\/\/core_engine_test:test_only@127\.0\.0\.1:5432\/core_engine_test$/m,
      );
      assert.match(workflow, /^ {8}run: npm run test:integration$/m);
    });

    test('R-DS-1/R-C-7: CI mantiene secreto, baseline y OpenAPI', () => {
      const workflow = readText(ciWorkflowPath);
      assert.match(workflow, /^\s*run: npm run security:secrets$/m);
      assert.match(workflow, /run: npm run baseline:guard -- --base-ref /);
      assert.match(
        workflow,
        /^\s*run: npm run swagger:export && git diff --exit-code docs\/openapi docs\/api$/m,
      );
      for (const service of SERVICES) {
        if (service === 'api-gateway') continue;
        for (const spec of ['public.openapi.json', 'internal.openapi.json']) {
          assert.ok(
            existsSync(join(repoRoot, 'docs', 'openapi', service, spec)),
            `falta docs/openapi/${service}/${spec}`,
          );
        }
      }
      for (const api of ['core-engine.openapi.json', 'core-engine.internal.openapi.json']) {
        assert.ok(existsSync(join(repoRoot, 'docs', 'api', api)), `falta docs/api/${api}`);
      }
    });

    test('R-MT-1/R-MT-2: no queda mutación Node; la deuda Rust es explícita', () => {
      assert.ok(
        !existsSync(join(repoRoot, WORKFLOWS_DIR, 'mutation.yml')),
        'no debe quedar mutation.yml de Stryker',
      );
      for (const service of SERVICES) {
        const dir = join(packagesRoot, service);
        assert.ok(!existsSync(join(dir, 'stryker.config.mjs')), `${service}: queda Stryker`);
      }
    });

    test('R-FL-3: los workflows fijan la timezone', () => {
      for (const workflow of ['ci.yml', 'e2e.yml', 'deploy.yml']) {
        const content = readText(join(repoRoot, WORKFLOWS_DIR, workflow));
        assert.match(content, /TZ:\s*America\/Managua/, `${workflow}: falta TZ fija`);
      }
    });

    test('R-RB-4: las llamadas internas Rust declaran timeout', () => {
      for (const file of [
        join(packagesRoot, 'identity-service', 'src', 'storage.rs'),
        join(packagesRoot, 'identity-service', 'src', 'stores.rs'),
      ]) {
        const codigo = readText(file);
        assert.match(codigo, /Client::builder\(\)/, `${file}: falta cliente HTTP`);
        assert.match(codigo, /\.timeout\(/, `${file}: falta timeout`);
      }
    });
  },
});
