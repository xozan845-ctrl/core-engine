/**
 * Runner del motor de reglas.
 *
 * Descubre las suites importándolas con el registro desactivado (metadata sin
 * efectos), valida el manifiesto (R-CI-7) y las ejecuta con `node:test` en
 * procesos aislados, con el reporter elegido. No conoce datos del proyecto:
 * todo lo específico llega por parámetro desde el CLI (composition root).
 */
import { spawn } from 'node:child_process';
import { relative, resolve } from 'node:path';
import {
  repoRoot,
  rulesDir as defaultRulesDir,
  scriptsRoot,
  toFileUrl,
} from './lib/paths.mjs';
import { walkFiles } from './lib/fs.mjs';
import { __setRegistration } from './suite.mjs';
import { printSuitePlan, printCoverage } from './report.mjs';

const REPORTERS = new Set(['spec', 'tap', 'dot', 'junit']);

export async function discoverSuites(rulesDir) {
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

export function validateManifest(suites, requiredSuites) {
  const ids = suites.map((suite) => suite.id);
  const duplicates = [...new Set(ids.filter((id, i) => ids.indexOf(id) !== i))];
  if (duplicates.length > 0) {
    throw new Error(`IDs de suite duplicados: ${duplicates.join(', ')}`);
  }

  const requiredIds = requiredSuites.map((suite) => suite.id);
  const duplicateRequired = [
    ...new Set(requiredIds.filter((id, i) => requiredIds.indexOf(id) !== i)),
  ];
  if (duplicateRequired.length > 0) {
    throw new Error(`IDs duplicados en el manifiesto: ${duplicateRequired.join(', ')}`);
  }

  const discovered = new Map(suites.map((suite) => [suite.id, suite]));
  const required = new Map(requiredSuites.map((suite) => [suite.id, suite]));

  const missing = requiredSuites
    .map((suite) => suite.id)
    .filter((id) => !discovered.has(id));
  if (missing.length > 0) {
    throw new Error(
      `Faltan suites obligatorias del manifiesto (scripts/config/manifest.mjs) (R-CI-7): ${missing.join(', ')}`,
    );
  }

  const unexpected = suites
    .map((suite) => suite.id)
    .filter((id) => !required.has(id));
  if (unexpected.length > 0) {
    throw new Error(
      `Suites descubiertas fuera del manifiesto (R-CI-7): ${unexpected.join(', ')}`,
    );
  }

  const mismatches = requiredSuites.flatMap((entry) => {
    const suite = discovered.get(entry.id);
    if (!suite) return [];
    const expected = resolve(scriptsRoot, entry.file);
    return suite.file === expected
      ? []
      : [`${entry.id}: manifiesto=${relative(repoRoot, expected)}, descubierto=${relative(repoRoot, suite.file)}`];
  });
  if (mismatches.length > 0) {
    throw new Error(`El archivo de la suite no coincide con el manifiesto:\n${mismatches.join('\n')}`);
  }
}

function selectSuites(suites, { only, mode } = {}) {
  return suites.filter((suite) => {
    if (mode && suite.mode !== mode) return false;
    if (only && suite.id !== only && suite.area !== only) return false;
    return true;
  });
}

export function execute(files, reporterName, { silent = false } = {}) {
  if (!REPORTERS.has(reporterName)) {
    throw new Error(
      `Reporter desconocido: ${reporterName}. Usa: ${[...REPORTERS].join(' | ')}`,
    );
  }

  return new Promise((resolveExecution, rejectExecution) => {
    const childEnv = { ...process.env };
    // Si el motor se prueba desde `node --test`, esta variable convertiría el
    // proceso aislado en hijo del test externo y filtraría la fixture fallida
    // al padre aunque su exit code se estuviera verificando correctamente.
    delete childEnv.NODE_TEST_CONTEXT;

    // La fase de descubrimiento ya importó las suites con el registro apagado.
    // Un proceso nuevo evita que la caché ESM convierta las assertions en un
    // falso verde al intentar cargar esos mismos módulos por segunda vez.
    const child = spawn(
      process.execPath,
      ['--test', `--test-reporter=${reporterName}`, ...files],
      {
        cwd: repoRoot,
        env: childEnv,
        stdio: silent ? 'ignore' : 'inherit',
      },
    );
    child.once('error', rejectExecution);
    child.once('exit', (code) => resolveExecution(code === 0 ? 0 : 1));
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
  silent = false,
} = {}) {
  const suites = await discoverSuites(rulesDir);
  validateManifest(suites, requiredSuites);

  const selected = selectSuites(suites, { only, mode });
  if (selected.length === 0) {
    console.error('No hay suites que coincidan con el filtro.');
    return 1;
  }

  if (!silent) printSuitePlan(selected);
  const code = await execute(
    selected.map((suite) => suite.file),
    reporter,
    { silent },
  );
  if (docsRulesRoot && !silent) printCoverage({ suites, rulesRoot: docsRulesRoot });
  return code;
}
