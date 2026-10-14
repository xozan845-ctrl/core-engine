/**
 * Runner del motor de reglas.
 *
 * Descubre las suites importándolas con el registro desactivado (metadata sin
 * efectos), valida el manifiesto (R-CI-7) y las ejecuta con `node:test` en
 * procesos aislados, con el reporter elegido. No conoce datos del proyecto:
 * todo lo específico llega por parámetro desde el CLI (composition root).
 */
import { run } from 'node:test';
import { spec, tap, dot, junit } from 'node:test/reporters';
import { relative } from 'node:path';
import { repoRoot, rulesDir as defaultRulesDir, toFileUrl } from './lib/paths.mjs';
import { walkFiles } from './lib/fs.mjs';
import { __setRegistration } from './suite.mjs';
import { printSuitePlan, printCoverage } from './report.mjs';

const REPORTERS = { spec, tap, dot, junit };

async function discoverSuites(rulesDir) {
  const files = walkFiles(rulesDir, (name) => name.endsWith('.test.mjs')).sort();

  __setRegistration(false);
  try {
    const suites = [];
    for (const file of files) {
      const module = await import(toFileUrl(file));
      if (!module.default) {
        throw new Error(
          `${relative(repoRoot, file)}: no exporta defineSuite(...) por defecto`,
        );
      }
      suites.push({ ...module.default, file });
    }
    return suites;
  } finally {
    __setRegistration(true);
  }
}

function validateManifest(suites, requiredSuites) {
  const discovered = new Set(suites.map((suite) => suite.id));

  const missing = requiredSuites
    .map((suite) => suite.id)
    .filter((id) => !discovered.has(id));
  if (missing.length > 0) {
    throw new Error(
      `Faltan suites obligatorias del manifiesto (scripts/config/manifest.mjs) (R-CI-7): ${missing.join(', ')}`,
    );
  }

  const ids = suites.map((suite) => suite.id);
  const duplicates = [...new Set(ids.filter((id, i) => ids.indexOf(id) !== i))];
  if (duplicates.length > 0) {
    throw new Error(`IDs de suite duplicados: ${duplicates.join(', ')}`);
  }
}

function selectSuites(suites, { only, mode } = {}) {
  return suites.filter((suite) => {
    if (mode && suite.mode !== mode) return false;
    if (only && suite.id !== only && suite.area !== only) return false;
    return true;
  });
}

function execute(files, reporterName) {
  const stream = run({ files });
  const reporter = REPORTERS[reporterName] ?? spec;
  stream.compose(reporter).pipe(process.stdout);

  let failed = false;
  stream.on('test:fail', () => {
    failed = true;
  });

  return new Promise((resolve) => {
    stream.on('end', () => resolve(failed ? 1 : 0));
  });
}

/**
 * Ejecuta las suites de reglas.
 * @returns {Promise<number>} Código de salida (0 si todo pasa).
 */
export async function runRules({
  rulesDir = defaultRulesDir,
  requiredSuites = [],
  docsRulesRoot,
  only,
  mode,
  reporter = 'spec',
} = {}) {
  const suites = await discoverSuites(rulesDir);
  validateManifest(suites, requiredSuites);

  const selected = selectSuites(suites, { only, mode });
  if (selected.length === 0) {
    console.error('No hay suites que coincidan con el filtro.');
    return 1;
  }

  printSuitePlan(selected);
  const code = await execute(selected.map((suite) => suite.file), reporter);
  if (docsRulesRoot) printCoverage({ suites, rulesRoot: docsRulesRoot });
  return code;
}
