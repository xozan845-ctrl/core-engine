#!/usr/bin/env node
/**
 * version-bump.mjs — alinea la versión de todos los workspaces con un release.
 *
 * Uso:
 *   node scripts/tools/version-bump.mjs 1.0.7
 *   npm run version:bump -- 1.0.7
 *
 * Actualiza el `package.json` raíz, los 12 workspaces y `package-lock.json`.
 * No commitea ni crea tags: el release sigue siendo un acto humano y revisable
 * (R-CD-1).
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';
import { parseTag } from '../engine/lib/semver.mjs';

function parseArgs(argv) {
  const options = { version: undefined, root: repoRoot, dryRun: false };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root') options.root = argv[++i];
    else if (arg.startsWith('--root=')) options.root = arg.slice('--root='.length);
    else if (arg === '--dry-run') options.dryRun = true;
    else if (arg === '--help' || arg === '-h') {
      console.log(`Uso: node scripts/tools/version-bump.mjs <X.Y.Z[-pre]> [opciones]

Opciones:
  --root <dir>   Raíz del repositorio (por defecto la del script).
  --dry-run      Muestra el comando sin modificar archivos.
  -h, --help     Muestra esta ayuda.`);
      process.exit(0);
    } else if (!arg.startsWith('-') && !options.version) options.version = arg;
    else throw new Error(`Argumento no reconocido: ${arg}`);
  }

  if (!options.version) throw new Error('Falta la versión <X.Y.Z[-pre]>.');
  if (!parseTag(`v${options.version}`)) {
    throw new Error(`'${options.version}' no es una versión semver válida.`);
  }
  return options;
}

/**
 * Resuelve los `package.json` del root y de los workspaces declarados en
 * `workspaces` (soporta patrones simples `dir/*`).
 */
function packageFiles(root) {
  const rootPackage = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  const files = [join(root, 'package.json')];
  const add = (file) => {
    if (existsSync(file) && !files.includes(file)) files.push(file);
  };

  for (const pattern of rootPackage.workspaces ?? []) {
    if (pattern.endsWith('/*')) {
      const base = join(root, pattern.slice(0, -2));
      if (!existsSync(base)) continue;
      for (const entry of readdirSync(base, { withFileTypes: true })) {
        if (entry.isDirectory()) add(join(base, entry.name, 'package.json'));
      }
    } else {
      add(join(root, pattern, 'package.json'));
    }
  }

  return files;
}

function setVersion(file, version) {
  const data = JSON.parse(readFileSync(file, 'utf8'));
  data.version = version;
  writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
}

function main() {
  const { version, root, dryRun } = parseArgs(process.argv.slice(2));
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  const files = packageFiles(root);

  console.log(`Versión objetivo: ${version}`);
  console.log(`Raíz: ${root}`);
  console.log(`Paquetes: ${files.length}`);

  if (dryRun) {
    for (const file of files) console.log(`[dry-run] version=${version} ${file}`);
    console.log(`[dry-run] ${npm} install --package-lock-only --ignore-scripts --prefer-offline`);
    return;
  }

  for (const file of files) setVersion(file, version);

  execFileSync(
    npm,
    ['install', '--package-lock-only', '--ignore-scripts', '--prefer-offline'],
    { cwd: root, stdio: 'inherit' },
  );

  console.log(`\n✅ Versiones actualizadas a ${version} en ${files.length} package.json + lockfile.`);
  console.log('   Revisa `git diff`, commitea el bump y solo entonces crea el tag anotado:');
  console.log(`   git tag -a v${version} -m "Release v${version}"`);
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
