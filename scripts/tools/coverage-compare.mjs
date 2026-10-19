#!/usr/bin/env node
/**
 * coverage-compare.mjs — gate de cobertura contra el baseline (R-COV-1).
 *
 * Uso:
 *   node scripts/tools/coverage-compare.mjs
 *   npm run coverage:compare
 *
 * Requiere que `npm test` haya generado `packages/<workspace>/coverage/coverage-summary.json`.
 * Falla si cualquier métrica de cualquier workspace queda por debajo del baseline
 * o si aparece un workspace sin baseline.
 */
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';
import {
  COVERAGE_METRICS,
  compareCoverage,
  readCoverageSummaries,
} from '../engine/lib/coverage.mjs';

function parseArgs(argv) {
  const options = {
    root: repoRoot,
    baseline: undefined,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root') options.root = argv[++i];
    else if (arg.startsWith('--root=')) options.root = arg.slice('--root='.length);
    else if (arg === '--baseline') options.baseline = argv[++i];
    else if (arg.startsWith('--baseline=')) options.baseline = arg.slice('--baseline='.length);
    else if (arg === '--help' || arg === '-h') {
      console.log(`Uso: node scripts/tools/coverage-compare.mjs [opciones]

Opciones:
  --root <dir>       Raíz del repositorio (por defecto la del script).
  --baseline <file>  Baseline alternativo (por defecto scripts/config/coverage-baseline.json).
  -h, --help         Muestra esta ayuda.`);
      process.exit(0);
    } else {
      throw new Error(`Argumento no reconocido: ${arg}`);
    }
  }

  options.baseline ??= join(options.root, 'scripts', 'config', 'coverage-baseline.json');
  return options;
}

function format(value) {
  return typeof value === 'number' ? value.toFixed(2) : '—';
}

function main() {
  const { root, baseline: baselinePath } = parseArgs(process.argv.slice(2));

  if (!existsSync(baselinePath)) {
    throw new Error(`No existe el baseline ${baselinePath}.`);
  }

  const baseline = JSON.parse(readFileSync(baselinePath, 'utf8'));
  const current = readCoverageSummaries(root);
  const problems = compareCoverage(current, baseline);

  const workspaces = [...new Set([...Object.keys(baseline), ...Object.keys(current)])].sort();
  const header = ['Workspace'.padEnd(32), ...COVERAGE_METRICS.map((m) => m.padEnd(18))].join(' ');
  console.log(header);
  for (const workspace of workspaces) {
    const row = [workspace.padEnd(32)];
    for (const metric of COVERAGE_METRICS) {
      row.push(
        `${format(current[workspace]?.[metric])}/${format(baseline[workspace]?.[metric])}`.padEnd(18),
      );
    }
    console.log(row.join(' '));
  }

  if (problems.length > 0) {
    console.error(`\n❌ Cobertura por debajo del baseline (R-COV-1):\n- ${problems.join('\n- ')}`);
    process.exit(1);
  }

  console.log(`\n✅ Cobertura igual o superior al baseline en ${workspaces.length} workspaces.`);
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
