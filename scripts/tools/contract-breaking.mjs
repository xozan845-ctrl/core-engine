#!/usr/bin/env node
/**
 * contract-breaking.mjs — cambios incompatibles en el contrato OpenAPI (R-C-7).
 *
 * Uso:
 *   npm run contract:breaking -- --base-ref origin/main
 *
 * Compara los specs agregados actuales contra los de la ref base. Un cambio
 * incompatible debe decidirse explícitamente (major/ADR), no colarse en un PR.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';
import { compareSpecs } from '../engine/lib/openapi-diff.mjs';

const SPECS = [
  'docs/api/core-engine.openapi.json',
  'docs/api/core-engine.internal.openapi.json',
];

/**
 * Une las superficies pública e interna. Un path que se mueve de una a otra no
 * es un cambio incompatible: el contrato agregado conserva la ruta.
 */
function combineSpecs(specs) {
  const combined = { openapi: '3.0.0', paths: {}, components: { schemas: {} } };
  for (const spec of specs) {
    Object.assign(combined.paths, spec.paths ?? {});
    Object.assign(combined.components.schemas, spec.components?.schemas ?? {});
  }
  return combined;
}

function readSpec(root, baseRef, file) {
  const currentPath = join(root, file);
  const current = existsSync(currentPath)
    ? JSON.parse(readFileSync(currentPath, 'utf8'))
    : null;
  const baseRaw = git(root, ['show', `${baseRef}:${file}`], { allowFailure: true });
  const base = baseRaw === null ? null : JSON.parse(baseRaw);
  return { current, base };
}

function git(root, args, { allowFailure = false } = {}) {
  try {
    return execFileSync('git', args, {
      cwd: root,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim();
  } catch (error) {
    if (allowFailure) return null;
    throw new Error(`git ${args.join(' ')} falló`, { cause: error });
  }
}

function parseArgs(argv) {
  const options = { root: repoRoot, baseRef: 'origin/main' };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root') options.root = argv[++i];
    else if (arg === '--base-ref') options.baseRef = argv[++i];
    else if (arg === '--help' || arg === '-h') {
      console.log('Uso: node scripts/tools/contract-breaking.mjs [--base-ref <ref>] [--root <dir>]');
      process.exit(0);
    } else {
      throw new Error(`Argumento no reconocido: ${arg}`);
    }
  }
  return options;
}

function main() {
  const { root, baseRef } = parseArgs(process.argv.slice(2));
  if (git(root, ['rev-parse', '--verify', baseRef], { allowFailure: true }) === null) {
    throw new Error(`No existe la referencia '${baseRef}'.`);
  }

  const currentSpecs = [];
  const baseSpecs = [];
  for (const file of SPECS) {
    const { current, base } = readSpec(root, baseRef, file);
    if (current) currentSpecs.push(current);
    if (base) baseSpecs.push(base);
  }

  const problems = compareSpecs(combineSpecs(baseSpecs), combineSpecs(currentSpecs));
  const baselinePath = join(root, 'docs', 'api', 'breaking-baseline.json');
  const allowed = new Set(
    existsSync(baselinePath)
      ? JSON.parse(readFileSync(baselinePath, 'utf8')).allow ?? []
      : [],
  );
  const pending = problems.filter((problem) => !allowed.has(problem));
  const suppressed = problems.filter((problem) => allowed.has(problem));

  for (const problem of suppressed) {
    console.warn(`⚠️  Excepción de baseline (R-C-7): ${problem}`);
  }

  if (pending.length > 0) {
    console.error(`❌ Cambios incompatibles en OpenAPI (R-C-7):\n- ${pending.join('\n- ')}`);
    process.exit(1);
  }

  console.log(
    `✅ Sin cambios incompatibles nuevos en OpenAPI respecto a '${baseRef}' (${suppressed.length} excepción(es) de baseline).`,
  );
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
