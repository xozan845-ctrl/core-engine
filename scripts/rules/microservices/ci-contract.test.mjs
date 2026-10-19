import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
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
  rules: ['R-MS-1', 'R-QA-1', 'R-COV-2', 'R-COV-4', 'R-CI-5', 'R-I-12'],
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
