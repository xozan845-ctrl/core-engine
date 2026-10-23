#!/usr/bin/env node
/**
 * db-migrations.mjs — verifica o actualiza el manifiesto de migraciones (R-DB-6).
 *
 * Uso:
 *   node scripts/tools/db-migrations.mjs            # --check
 *   node scripts/tools/db-migrations.mjs --update
 *   npm run db:migrations
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';
import {
  buildMigrationManifest,
  checkMigrationManifest,
} from '../engine/lib/db.mjs';

function parseArgs(argv) {
  const options = {
    root: repoRoot,
    manifest: undefined,
    update: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root') options.root = argv[++i];
    else if (arg.startsWith('--root=')) options.root = arg.slice('--root='.length);
    else if (arg === '--manifest') options.manifest = argv[++i];
    else if (arg.startsWith('--manifest=')) options.manifest = arg.slice('--manifest='.length);
    else if (arg === '--update') options.update = true;
    else if (arg === '--check') options.update = false;
    else if (arg === '--help' || arg === '-h') {
      console.log(`Uso: node scripts/tools/db-migrations.mjs [opciones]

Opciones:
  --check             Verifica el manifiesto (por defecto).
  --update            Regenera scripts/config/db-migrations.json.
  --manifest <file>   Manifiesto alternativo.
  --root <dir>        Raíz del repositorio (por defecto la del script).
  -h, --help          Muestra esta ayuda.`);
      process.exit(0);
    } else {
      throw new Error(`Argumento no reconocido: ${arg}`);
    }
  }

  options.manifest ??= join(options.root, 'scripts', 'config', 'db-migrations.json');
  return options;
}

function main() {
  const { root, manifest: manifestPath, update } = parseArgs(process.argv.slice(2));

  if (update) {
    const manifest = buildMigrationManifest(root);
    writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
    console.log(`✅ Manifiesto actualizado en ${manifestPath} (${Object.keys(manifest).length} migraciones).`);
    return;
  }

  if (!existsSync(manifestPath)) {
    throw new Error(`No existe el manifiesto ${manifestPath}.`);
  }

  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  const problems = checkMigrationManifest(root, manifest);

  if (problems.length > 0) {
    console.error(`❌ Migraciones fuera del manifiesto (R-DB-6):\n- ${problems.join('\n- ')}`);
    console.error('\nSi la migración es nueva y deliberada: npm run db:migrations -- --update');
    process.exit(1);
  }

  console.log(`✅ ${Object.keys(manifest).length} migraciones íntegras y registradas.`);
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
