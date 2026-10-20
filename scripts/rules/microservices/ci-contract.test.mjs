import { existsSync, readdirSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { readJson, readText, walkFiles } from '../../engine/lib/fs.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { COVERAGE_THRESHOLDS } from '../../config/coverage.mjs';
import {
  PACKAGES_DIR,
  SERVICES,
  SERVICE_TEST_EXCEPTIONS,
  WORKFLOWS_DIR,
} from '../../config/project.mjs';

const packagesRoot = join(repoRoot, PACKAGES_DIR);
const rootPackage = readJson(join(repoRoot, 'package.json'));
const coverageDoc = join(
  repoRoot,
  'docs',
  'rules',
  'test',
  '06-estandares-cobertura.md',
);

/**
 * Extrae el baseline documentado de la tabla de umbrales por workspace. La
 * tabla es la referencia normativa; `coverage.mjs` no puede quedar por debajo.
 */
function documentedCoverageThresholds() {
  const thresholds = new Map();
  for (const line of readText(coverageDoc).split('\n')) {
    const match =
      /^\s*\|\s*`([a-z0-9-]+)`\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*$/.exec(
        line,
      );
    if (!match) continue;
    thresholds.set(match[1], {
      lines: Number(match[2]),
      branches: Number(match[3]),
      functions: Number(match[4]),
    });
  }
  return thresholds;
}

const packageDir = (workspace) => join(packagesRoot, workspace);
const packageJson = (workspace) =>
  readJson(join(packageDir(workspace), 'package.json'));

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

function workspacesInScript(script, task) {
  const coverageArgs = task === 'test' ? '\\s+--\\s+--coverage' : '';
  const pattern = new RegExp(
    `npm run ${task} -w @core/([a-z0-9-]+)${coverageArgs}`,
    'g',
  );
  return [...script.matchAll(pattern)].map((match) => match[1]);
}

export default defineSuite({
  id: 'microservices-ci-contract',
  area: 'test',
  rules: ['R-MS-1', 'R-QA-1', 'R-COV-1', 'R-COV-2', 'R-COV-4', 'R-CI-5', 'R-I-12', 'R-DS-1', 'R-C-7'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-MS-1: el inventario declara exactamente los servicios desplegables', () => {
      assert.deepEqual(
        sorted(SERVICES),
        sorted(deployedWorkspaceDirs()),
        'SERVICES debe coincidir con los directorios packages/* que tienen Dockerfile',
      );
      assert.equal(
        new Set(SERVICES).size,
        SERVICES.length,
        'SERVICES no admite entradas duplicadas',
      );
    });

    test('R-MS-1: cada paquete de servicio expone los scripts mínimos de runtime', () => {
      const problems = [];
      for (const service of SERVICES) {
        const pkg = packageJson(service);
        if (pkg.private !== true) problems.push(`${service}: private debe ser true`);
        if (pkg.main !== 'dist/main.js') problems.push(`${service}: main debe ser dist/main.js`);
        if (!/^tsc -p tsconfig(?:\.build)?\.json$/.test(pkg.scripts?.build ?? '')) {
          problems.push(`${service}: script build no compila un tsconfig válido`);
        }
        if (pkg.scripts?.start !== 'node dist/main.js') {
          problems.push(`${service}: script start debe ejecutar node dist/main.js`);
        }
        if (pkg.dependencies?.['@core/shared'] !== 'file:../shared') {
          problems.push(`${service}: debe depender de @core/shared por file:../shared`);
        }
      }
      assert.deepEqual(problems, [], problems.join('\n'));
    });

    test('R-CI-5: el build raíz incluye una sola vez todos los servicios', () => {
      assert.equal(
        rootPackage.scripts?.['build:shared'],
        'npm run build -w @core/shared',
        'build:shared debe compilar la librería transversal antes de los servicios',
      );
      assert.deepEqual(
        sorted(workspacesInScript(rootPackage.scripts?.build ?? '', 'build')),
        sorted(SERVICES),
        'el script build raíz debe enumerar exactamente una vez todos los servicios',
      );
    });

    test('R-QA-1: la matriz test raíz cubre cada workspace no exceptuado', () => {
      const expected = [
        'shared',
        ...SERVICES.filter((service) => !SERVICE_TEST_EXCEPTIONS.includes(service)),
      ];
      const actual = workspacesInScript(rootPackage.scripts?.test ?? '', 'test');
      assert.deepEqual(
        sorted(actual),
        sorted(expected),
        'el script test raíz debe ejecutar con coverage cada workspace exigible, sin duplicados',
      );
    });

    test('R-QA-1/R-COV-2: cada workspace exigible tiene runner, config y specs', () => {
      const workspaces = [
        'shared',
        ...SERVICES.filter((service) => !SERVICE_TEST_EXCEPTIONS.includes(service)),
      ];
      const problems = [];

      for (const workspace of workspaces) {
        const pkg = packageJson(workspace);
        const configPath = join(packageDir(workspace), 'vitest.config.ts');
        const specs = walkFiles(
          join(packageDir(workspace), 'src'),
          (name) => name.endsWith('.spec.ts'),
        );

        if (pkg.scripts?.test !== 'vitest run') {
          problems.push(`${workspace}: falta script test = vitest run`);
        }
        if (!existsSync(configPath)) {
          problems.push(`${workspace}: falta vitest.config.ts`);
        }
        if (specs.length === 0) {
          problems.push(`${workspace}: no contiene archivos *.spec.ts`);
        }
      }

      assert.deepEqual(problems, [], problems.join('\n'));
    });

    test('R-COV-4: la fuente central declara umbrales para cada workspace', () => {
      const workspaces = [
        'shared',
        ...SERVICES.filter((service) => !SERVICE_TEST_EXCEPTIONS.includes(service)),
      ];

      assert.deepEqual(
        sorted(Object.keys(COVERAGE_THRESHOLDS)),
        sorted(workspaces),
        'scripts/config/coverage.mjs debe declarar exactamente los workspaces con suite',
      );

      const problems = [];
      for (const [workspace, thresholds] of Object.entries(COVERAGE_THRESHOLDS)) {
        for (const metric of ['lines', 'branches', 'functions']) {
          if (!Number.isInteger(thresholds[metric])) {
            problems.push(`${workspace}: falta el umbral de ${metric}`);
          }
        }
      }
      assert.deepEqual(problems, [], problems.join('\n'));
    });

    test('R-COV-1: la fuente central no queda por debajo del baseline documentado', () => {
      const documented = documentedCoverageThresholds();
      const problems = [];

      for (const [workspace, floor] of documented) {
        const current = COVERAGE_THRESHOLDS[workspace];
        if (!current) {
          problems.push(`${workspace}: documentado pero sin umbral central`);
          continue;
        }
        for (const metric of ['lines', 'branches', 'functions']) {
          if (current[metric] < floor[metric]) {
            problems.push(
              `${workspace}: ${metric} ${current[metric]} < baseline documentado ${floor[metric]}`,
            );
          }
        }
      }

      for (const workspace of Object.keys(COVERAGE_THRESHOLDS)) {
        if (!documented.has(workspace)) {
          problems.push(`${workspace}: sin fila en la tabla de docs/rules/test/06`);
        }
      }

      assert.deepEqual(
        problems,
        [],
        `Umbrales por debajo del baseline o desalineados (R-COV-1):\n${problems.join('\n')}`,
      );
    });

    test('R-COV-2: los services/consumers sin spec coinciden con el baseline', () => {
      const workspaces = [
        'shared',
        ...SERVICES.filter((service) => !SERVICE_TEST_EXCEPTIONS.includes(service)),
      ];
      const baseline = new Set(
        readJson(join(repoRoot, 'scripts', 'config', 'spec-baseline.json')),
      );
      const problems = [];
      const missing = new Set();

      for (const workspace of workspaces) {
        const sourceDir = join(packageDir(workspace), 'src');
        if (!existsSync(sourceDir)) continue;

        for (const file of walkFiles(
          sourceDir,
          (name) => /\.(service|consumer)\.ts$/.test(name) && !name.endsWith('.spec.ts'),
        )) {
          const path = relative(repoRoot, file).split(sep).join('/');
          const spec = file.replace(/\.ts$/, '.spec.ts');

          if (!existsSync(spec)) {
            missing.add(path);
            if (!baseline.has(path)) {
              problems.push(`${path}: sin spec y no está en el baseline`);
            }
          } else if (baseline.has(path)) {
            problems.push(`${path}: ya tiene spec; elimina la excepción del baseline`);
          }
        }
      }

      for (const path of baseline) {
        if (!existsSync(join(repoRoot, path))) {
          problems.push(`${path}: el baseline apunta a un archivo que no existe`);
        } else if (!missing.has(path)) {
          problems.push(`${path}: baseline obsoleto (ya no falta el spec)`);
        }
      }

      assert.deepEqual(
        problems,
        [],
        `Specs faltantes fuera del baseline (R-COV-2):\n${problems.join('\n')}`,
      );
    });

    test('R-DS-1: CI escanea secretos de alta confianza en archivos versionados', () => {
      const workflow = readText(join(repoRoot, WORKFLOWS_DIR, 'ci.yml'));
      assert.match(
        workflow,
        /^\s*run: npm run security:secrets$/m,
        'CI debe ejecutar security:secrets (R-DS-1)',
      );
      assert.ok(
        existsSync(join(repoRoot, 'scripts', 'tools', 'secrets-check.mjs')),
        'falta scripts/tools/secrets-check.mjs',
      );
    });

    test('R-C-7: CI bloquea cambios incompatibles en OpenAPI', () => {
      const workflow = readText(join(repoRoot, WORKFLOWS_DIR, 'ci.yml'));
      assert.match(
        workflow,
        /^\s*run: npm run contract:breaking -- --base-ref origin\/main$/m,
        'CI debe ejecutar contract:breaking contra origin/main (R-C-7)',
      );
      assert.ok(
        existsSync(join(repoRoot, 'scripts', 'tools', 'contract-breaking.mjs')),
        'falta scripts/tools/contract-breaking.mjs',
      );
    });

    test('R-COV-1: CI compara la cobertura real contra el baseline', () => {
      const workflow = readText(join(repoRoot, WORKFLOWS_DIR, 'ci.yml'));
      assert.match(
        workflow,
        /^\s*run: npm run coverage:compare$/m,
        'CI debe ejecutar coverage:compare después de npm test (R-COV-1)',
      );

      const workspaces = [
        'shared',
        ...SERVICES.filter((service) => !SERVICE_TEST_EXCEPTIONS.includes(service)),
      ];
      const baseline = readJson(
        join(repoRoot, 'scripts', 'config', 'coverage-baseline.json'),
      );

      assert.deepEqual(
        sorted(Object.keys(baseline)),
        sorted(workspaces),
        'el baseline debe declarar exactamente los workspaces con suite',
      );

      const problems = [];
      for (const [workspace, metrics] of Object.entries(baseline)) {
        for (const metric of ['lines', 'branches', 'functions', 'statements']) {
          if (typeof metrics[metric] !== 'number') {
            problems.push(`${workspace}: baseline sin la métrica '${metric}'`);
          }
        }
      }
      assert.deepEqual(problems, [], problems.join('\n'));
    });

    test('R-COV-4: cada config Vitest aplica alcance y consume los thresholds centrales', () => {
      const workspaces = [
        'shared',
        ...SERVICES.filter((service) => !SERVICE_TEST_EXCEPTIONS.includes(service)),
      ];
      const problems = [];

      for (const workspace of workspaces) {
        const configPath = join(packageDir(workspace), 'vitest.config.ts');
        if (!existsSync(configPath)) continue;
        const config = readText(configPath);
        for (const [label, pattern] of [
          ['specs', /include:\s*\['src\/\*\*\/\*\.spec\.ts'\]/],
          ['lógica ejecutable', /include:\s*\['src\/\*\*\/\*\.ts'\]/],
          [
            'import de la fuente central',
            /import \{ coverageThresholdsFor \} from '\.\.\/\.\.\/scripts\/config\/coverage\.mjs';/,
          ],
          [
            'thresholds centrales',
            new RegExp(`thresholds:\\s*coverageThresholdsFor\\(\\s*'${workspace}'\\s*\\)`),
          ],
          ['reporte al fallar', /reportOnFailure:\s*true/],
        ]) {
          if (!pattern.test(config)) problems.push(`${workspace}: cobertura sin ${label}`);
        }
      }

      assert.deepEqual(problems, [], problems.join('\n'));
    });

    test('R-QA-1: las excepciones son explícitas, únicas y todavía no tienen suite', () => {
      assert.equal(
        new Set(SERVICE_TEST_EXCEPTIONS).size,
        SERVICE_TEST_EXCEPTIONS.length,
        'SERVICE_TEST_EXCEPTIONS no admite duplicados',
      );

      const problems = [];
      for (const service of SERVICE_TEST_EXCEPTIONS) {
        if (!SERVICES.includes(service)) {
          problems.push(`${service}: no existe en SERVICES`);
          continue;
        }
        const pkg = packageJson(service);
        const specs = walkFiles(
          join(packageDir(service), 'src'),
          (name) => name.endsWith('.spec.ts'),
        );
        if (pkg.scripts?.test) problems.push(`${service}: ya tiene script test; elimina la excepción`);
        if (existsSync(join(packageDir(service), 'vitest.config.ts'))) {
          problems.push(`${service}: ya tiene vitest.config.ts; elimina la excepción`);
        }
        if (specs.length > 0) problems.push(`${service}: ya tiene specs; elimina la excepción`);
      }
      assert.deepEqual(problems, [], problems.join('\n'));
    });

    test('R-CI-5: el workflow delega test, smoke y build en los comandos raíz', () => {
      const workflow = readText(join(repoRoot, WORKFLOWS_DIR, 'ci.yml'));
      assert.match(workflow, /^\s*run: npm test$/m, 'CI debe ejecutar npm test');
      assert.equal(rootPackage.scripts?.['test:smoke'], 'node --test scripts/tools/*.test.mjs');
      assert.ok(existsSync(join(repoRoot, 'scripts', 'tools', 'smoke.test.mjs')));
      assert.match(workflow, /^\s*run: npm run test:smoke$/m, 'CI debe probar el smoke');
      assert.match(workflow, /^\s*run: npm run build$/m, 'CI debe ejecutar npm run build');
    });

    test('R-I-12: la integración de catálogo usa PostgreSQL efímero en CI', () => {
      const workflow = readText(join(repoRoot, WORKFLOWS_DIR, 'ci.yml'));
      assert.equal(rootPackage.scripts?.['test:integration'], 'node --test scripts/integration/*.test.mjs');
      assert.ok(existsSync(join(repoRoot, 'scripts', 'integration', 'catalog-stock.test.mjs')));
      assert.match(workflow, /^ {2}integration-catalog:$/m);
      assert.match(workflow, /^ {8}image: postgres:16-alpine$/m);
      assert.match(workflow, /^ {6}TEST_DATABASE_URL: postgres:\/\/core_engine_test:test_only@127\.0\.0\.1:5432\/core_engine_test$/m);
      assert.match(workflow, /^ {8}run: npm run test:integration$/m);
      assert.match(workflow, /^ {4}needs: \[test, lint, integration-catalog\]$/m);
    });
  },
});
