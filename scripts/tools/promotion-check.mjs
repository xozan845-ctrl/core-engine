#!/usr/bin/env node
/**
 * promotion-check.mjs — valida una rama de promoción por delta (R-GE-7).
 *
 * Uso:
 *   node scripts/tools/promotion-check.mjs \
 *     --source origin/develop --destination origin/staging --branch HEAD
 *
 * Comprueba que la rama:
 *   1. nace de la rama destino (el destino es ancestro);
 *   2. tiene exactamente el mismo árbol que la rama origen.
 *
 * Así una promoción no arrastra historiales duplicados ni deja contenido fuera.
 */
import { execFileSync } from 'node:child_process';
import { repoRoot } from '../engine/lib/paths.mjs';

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
  const options = {
    root: repoRoot,
    source: 'origin/develop',
    destination: 'origin/staging',
    branch: 'HEAD',
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root') options.root = argv[++i];
    else if (arg === '--source') options.source = argv[++i];
    else if (arg === '--destination') options.destination = argv[++i];
    else if (arg === '--branch') options.branch = argv[++i];
    else if (arg === '--help' || arg === '-h') {
      console.log(`Uso: node scripts/tools/promotion-check.mjs [opciones]

Opciones:
  --source <ref>       Rama origen (por defecto origin/develop).
  --destination <ref>  Rama destino (por defecto origin/staging).
  --branch <ref>       Rama de promoción a validar (por defecto HEAD).
  --root <dir>         Raíz del repositorio.
  -h, --help           Muestra esta ayuda.`);
      process.exit(0);
    } else {
      throw new Error(`Argumento no reconocido: ${arg}`);
    }
  }

  return options;
}

function main() {
  const { root, source, destination, branch } = parseArgs(process.argv.slice(2));

  for (const ref of [source, destination, branch]) {
    if (git(root, ['rev-parse', '--verify', ref], { allowFailure: true }) === null) {
      throw new Error(`No existe la referencia '${ref}'.`);
    }
  }

  if (
    git(root, ['merge-base', '--is-ancestor', destination, branch], {
      allowFailure: true,
    }) === null
  ) {
    throw new Error(
      `La rama '${branch}' no nace de '${destination}': crea la promoción desde el destino (R-GE-7).`,
    );
  }

  const diff = git(root, ['diff', '--name-status', source, branch]);
  if (diff) {
    throw new Error(
      `La rama '${branch}' no tiene el mismo contenido que '${source}' (R-GE-7):\n${diff}`,
    );
  }

  console.log(`✅ Promoción válida: '${branch}' parte de '${destination}' y su árbol es idéntico a '${source}'.`);
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
