#!/usr/bin/env node
/**
 * db-rls.mjs — cobertura de Row Level Security (R-SB-2).
 *
 * Uso:
 *   node scripts/tools/db-rls.mjs            # --check
 *   node scripts/tools/db-rls.mjs --update
 *   npm run db:rls
 *
 * El baseline `scripts/config/rls-baseline.json` enumera la deuda existente
 * (tablas sin RLS/policy). El gate falla si aparece una tabla nueva sin RLS o
 * si una tabla del baseline ya está cubierta y no se retiró de la lista.
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';
import { checkRlsCoverage, missingRlsTables } from '../engine/lib/db.mjs';

function parseArgs(argv) {
  const options = {
    root: repoRoot,
    baseline: undefined,
    update: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root') options.root = argv[++i];
    else if (arg.startsWith('--root=')) options.root = arg.slice('--root='.length);
    else if (arg === '--baseline') options.baseline = argv[++i];
    else if (arg.startsWith('--baseline=')) options.baseline = arg.slice('--baseline='.length);
    else if (arg === '--update') options.update = true;
    else if (arg === '--check') options.update = false;
    else if (arg === '--help' || arg === '-h') {
      console.log(`Uso: node scripts/tools/db-rls.mjs [opciones]

Opciones:
  --check             Verifica cobertura RLS contra el baseline (por defecto).
  --update            Regenera scripts/config/rls-baseline.json con la deuda actual.
  --baseline <file>   Baseline alternativo.
  --root <dir>        Raíz del repositorio (por defecto la del script).
  -h, --help          Muestra esta ayuda.`);
      process.exit(0);
    } else {
      throw new Error(`Argumento no reconocido: ${arg}`);
    }
  }

  options.baseline ??= join(options.root, 'scripts', 'config', 'rls-baseline.json');
  return options;
}

function main() {
  const { root, baseline: baselinePath, update } = parseArgs(process.argv.slice(2));

  if (update) {
    const tables = missingRlsTables(root);
    writeFileSync(baselinePath, `${JSON.stringify(tables, null, 2)}\n`);
    console.log(`✅ Baseline RLS actualizado en ${baselinePath} (${tables.length} tablas).`);
    return;
  }

  if (!existsSync(baselinePath)) {
    throw new Error(`No existe el baseline ${baselinePath}.`);
  }

  const baseline = JSON.parse(readFileSync(baselinePath, 'utf8'));
  const problems = checkRlsCoverage(root, baseline);

  if (problems.length > 0) {
    console.error(`❌ Cobertura RLS fuera del baseline (R-SB-2):\n- ${problems.join('\n- ')}`);
    process.exit(1);
  }

  console.log(`✅ ${baseline.length} tablas en el baseline RLS; ninguna tabla nueva sin RLS.`);
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
