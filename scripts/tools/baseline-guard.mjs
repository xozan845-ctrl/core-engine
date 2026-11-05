#!/usr/bin/env node
/**
 * baseline-guard.mjs — evita relajar controles versionados en un PR.
 *
 * Los baselines son útiles para representar deuda existente, pero no pueden
 * crecer silenciosamente en el mismo cambio que introduce la regresión.
 * Las mejoras sí pueden reducirlos y las migraciones nuevas pueden agregar
 * una entrada de hash, pero nunca se puede editar/eliminar una entrada ya
 * registrada.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';

const BASELINE_FILES = {
  coverage: 'scripts/config/coverage-baseline.json',
  specs: 'scripts/config/spec-baseline.json',
  rls: 'scripts/config/rls-baseline.json',
  migrations: 'scripts/config/db-migrations.json',
  breaking: 'docs/api/breaking-baseline.json',
};

function parseArgs(argv) {
  const options = { root: repoRoot, baseRef: 'origin/main' };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root') options.root = argv[++i];
    else if (arg.startsWith('--root=')) options.root = arg.slice('--root='.length);
    else if (arg === '--base-ref') options.baseRef = argv[++i];
    else if (arg.startsWith('--base-ref=')) options.baseRef = arg.slice('--base-ref='.length);
    else if (arg === '--help' || arg === '-h') {
      console.log('Uso: node scripts/tools/baseline-guard.mjs [--base-ref <ref>] [--root <dir>]');
      process.exit(0);
    } else {
      throw new Error(`Argumento no reconocido: ${arg}`);
    }
  }
  return options;
}

function gitShow(root, ref, file) {
  try {
    return execFileSync('git', ['show', `${ref}:${file}`], {
      cwd: root,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });
  } catch {
    return null;
  }
}

function readJson(text, file) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${file}: JSON inválido`, { cause: error });
  }
}

function readCurrent(root, file) {
  const path = join(root, file);
  if (!existsSync(path)) throw new Error(`No existe el baseline actual: ${file}`);
  return readJson(readFileSync(path, 'utf8'), file);
}

function compareCoverage(current, previous) {
  const problems = [];
  for (const [workspace, metrics] of Object.entries(previous)) {
    if (!current[workspace]) {
      problems.push(`coverage: se eliminó el workspace ${workspace}`);
      continue;
    }
    for (const [metric, value] of Object.entries(metrics)) {
      if (typeof current[workspace][metric] !== 'number') {
        problems.push(`coverage.${workspace}: falta la métrica ${metric}`);
      } else if (current[workspace][metric] < value) {
        problems.push(
          `coverage.${workspace}.${metric}: ${current[workspace][metric]} < ${value}`,
        );
      }
    }
  }
  return problems;
}

function compareDebtSet(name, current, previous) {
  const oldSet = new Set(previous);
  return current
    .filter((entry) => !oldSet.has(entry))
    .map((entry) => `${name}: nueva deuda/excepción no permitida: ${entry}`);
}

function compareMigrations(current, previous) {
  const problems = [];
  for (const [file, hash] of Object.entries(previous)) {
    if (!(file in current)) problems.push(`migrations: se eliminó ${file}`);
    else if (current[file] !== hash) problems.push(`migrations: hash alterado para ${file}`);
  }
  return problems;
}

/**
 * Compara documentos ya parseados. Es exportada para probar la política sin
 * depender de un repositorio Git ni de la red.
 */
export function compareBaselines(current, previous) {
  const problems = [
    ...compareCoverage(current.coverage, previous.coverage),
    ...compareDebtSet('specs', current.specs, previous.specs),
    ...compareDebtSet('rls', current.rls, previous.rls),
    ...compareMigrations(current.migrations, previous.migrations),
    ...compareDebtSet('breaking', current.breaking.allow ?? [], previous.breaking.allow ?? []),
  ];
  return problems;
}

function loadDocuments(root, baseRef) {
  const current = {};
  const previous = {};
  for (const [name, file] of Object.entries(BASELINE_FILES)) {
    current[name] = readCurrent(root, file);
    const baseText = gitShow(root, baseRef, file);
    if (baseText === null) {
      previous[name] = current[name];
    } else {
      previous[name] = readJson(baseText, file);
    }
  }
  return { current, previous };
}

function main() {
  const { root, baseRef } = parseArgs(process.argv.slice(2));
  const { current, previous } = loadDocuments(root, baseRef);
  const problems = compareBaselines(current, previous);
  if (problems.length > 0) {
    console.error(`❌ Baselines relajados respecto a '${baseRef}':\n- ${problems.join('\n- ')}`);
    process.exit(1);
  }
  console.log(`✅ Baselines no retroceden respecto a '${baseRef}'.`);
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
