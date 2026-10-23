#!/usr/bin/env node
/**
 * coverage-baseline.mjs — actualiza el baseline de cobertura (R-COV-1).
 *
 * Uso:
 *   npm run coverage:baseline
 *
 * Escribe `scripts/config/coverage-baseline.json` a partir de los reportes
 * `packages/<workspace>/coverage/coverage-summary.json`. El cambio se revisa y se
 * commitea como parte del PR que mejora la cobertura; el gate nunca lo sube
 * automáticamente.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';
import { readCoverageSummaries } from '../engine/lib/coverage.mjs';

function parseArgs(argv) {
  const options = {
    root: repoRoot,
    baseline: undefined,
    dryRun: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root') options.root = argv[++i];
    else if (arg.startsWith('--root=')) options.root = arg.slice('--root='.length);
    else if (arg === '--baseline') options.baseline = argv[++i];
    else if (arg.startsWith('--baseline=')) options.baseline = arg.slice('--baseline='.length);
    else if (arg === '--dry-run') options.dryRun = true;
    else if (arg === '--help' || arg === '-h') {
      console.log(`Uso: node scripts/tools/coverage-baseline.mjs [opciones]

Opciones:
  --root <dir>       Raíz del repositorio (por defecto la del script).
  --baseline <file>  Baseline alternativo (por defecto scripts/config/coverage-baseline.json).
  --dry-run          Imprime el JSON sin escribir el archivo.
  -h, --help         Muestra esta ayuda.`);
      process.exit(0);
    } else {
      throw new Error(`Argumento no reconocido: ${arg}`);
    }
  }

  options.baseline ??= join(options.root, 'scripts', 'config', 'coverage-baseline.json');
  return options;
}

function main() {
  const { root, baseline: baselinePath, dryRun } = parseArgs(process.argv.slice(2));
  const content = `${JSON.stringify(readCoverageSummaries(root), null, 2)}\n`;

  if (dryRun) {
    process.stdout.write(content);
    return;
  }

  const previous = (() => {
    try {
      return readFileSync(baselinePath, 'utf8');
    } catch {
      return null;
    }
  })();

  if (previous === content) {
    console.log('✅ El baseline ya está al día; no hay cambios.');
    return;
  }

  writeFileSync(baselinePath, content);
  console.log(`✅ Baseline actualizado en ${baselinePath}.`);
  console.log('   Revisa `git diff` y commitea el cambio junto con la mejora de cobertura.');
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
