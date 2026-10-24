#!/usr/bin/env node
/**
 * release-check.mjs — valida un tag de release antes de publicar (R-CD-1/R-CD-2).
 *
 * Uso:
 *   node scripts/tools/release-check.mjs --tag v1.0.7
 *   npm run release:check -- --tag v1.0.7
 *
 * Comprueba:
 *   - formato semver `vX.Y.Z[-pre]`
 *   - tag existente y anotado (no lightweight)
 *   - commit del tag ancestro de `origin/main` (o `--main-ref`)
 *   - tag estrictamente mayor que el último tag de release
 *   - desde `VERSION_MATCH_FROM`, package.json/lockfile iguales a `X.Y.Z`
 *   - con `--check-remote`, que un tag ya publicado no apunte a otro commit
 */
import { execFileSync } from 'node:child_process';
import { repoRoot } from '../engine/lib/paths.mjs';
import { compareSemver, parseTag } from '../engine/lib/semver.mjs';
import { SERVICES } from '../config/project.mjs';
import {
  DEFAULT_MAIN_REF,
  RELEASE_TAG_PATTERN,
  VERSION_MATCH_FROM,
} from '../config/release.mjs';

function printHelp() {
  console.log(`Uso: node scripts/tools/release-check.mjs --tag <vX.Y.Z[-pre]> [opciones]

Opciones:
  --tag <tag>        Tag de release a validar (obligatorio).
  --main-ref <ref>   Rama de producción (por defecto ${DEFAULT_MAIN_REF}).
  --check-remote     Rechaza un tag ya publicado que apunte a otro commit.
  --root <dir>       Raíz del repositorio (por defecto la del script).
  -h, --help         Muestra esta ayuda.`);
}

function parseArgs(argv) {
  const options = {
    tag: undefined,
    mainRef: DEFAULT_MAIN_REF,
    root: repoRoot,
    checkRemote: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--tag') options.tag = argv[++i];
    else if (arg.startsWith('--tag=')) options.tag = arg.slice('--tag='.length);
    else if (arg === '--main-ref') options.mainRef = argv[++i];
    else if (arg.startsWith('--main-ref=')) options.mainRef = arg.slice('--main-ref='.length);
    else if (arg === '--root') options.root = argv[++i];
    else if (arg.startsWith('--root=')) options.root = arg.slice('--root='.length);
    else if (arg === '--check-remote') options.checkRemote = true;
    else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else if (!arg.startsWith('-') && !options.tag) options.tag = arg;
    else throw new Error(`Argumento no reconocido: ${arg}`);
  }

  if (!options.tag) throw new Error('Falta --tag <vX.Y.Z> (o el tag como primer argumento).');
  return options;
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
    const detail = error.stderr?.toString().trim();
    throw new Error(`git ${args.join(' ')} falló${detail ? `: ${detail}` : ''}`, {
      cause: error,
    });
  }
}

function readJsonAt(root, sha, file) {
  const content = git(root, ['show', `${sha}:${file}`], { allowFailure: true });
  if (content === null) throw new Error(`No existe ${file} en el commit ${sha}.`);
  try {
    return JSON.parse(content);
  } catch (error) {
    throw new Error(`${file} del commit ${sha} no es JSON válido.`, { cause: error });
  }
}

function checkVersionMatch(root, sha, tag) {
  const expected = tag.slice(1);
  const workspaces = ['shared', ...SERVICES];
  const files = ['package.json', ...workspaces.map((ws) => `packages/${ws}/package.json`)];
  const problems = [];

  for (const file of files) {
    const pkg = readJsonAt(root, sha, file);
    if (pkg.version !== expected) {
      problems.push(`${file}: version '${pkg.version}' != '${expected}'`);
    }
  }

  const lock = readJsonAt(root, sha, 'package-lock.json');
  for (const [label, value] of [
    ['version', lock.version],
    ['packages[""].version', lock.packages?.['']?.version],
  ]) {
    if (value !== expected) {
      problems.push(`package-lock.json ${label}: '${value}' != '${expected}'`);
    }
  }

  for (const ws of workspaces) {
    for (const key of [`packages/${ws}`, `node_modules/@core/${ws}`]) {
      const value = lock.packages?.[key]?.version;
      if (value && value !== expected) {
        problems.push(`package-lock.json ${key}.version: '${value}' != '${expected}'`);
      }
    }
  }

  if (problems.length > 0) {
    throw new Error(
      `Las versiones no coinciden con ${tag} (R-CD-1):\n- ${problems.join('\n- ')}`,
    );
  }

  return files.length;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const { tag, mainRef, root, checkRemote } = options;

  if (!RELEASE_TAG_PATTERN.test(tag)) {
    throw new Error(`'${tag}' no es un tag semver válido v<major>.<minor>.<patch>[-pre].`);
  }

  const parsed = parseTag(tag);
  if (!parsed) throw new Error(`No se pudo interpretar el tag '${tag}'.`);

  const tagRef = `refs/tags/${tag}`;
  if (git(root, ['rev-parse', '--verify', tagRef], { allowFailure: true }) === null) {
    throw new Error(`El tag '${tag}' no existe localmente.`);
  }

  const objectType = git(root, ['cat-file', '-t', tagRef]);
  if (objectType !== 'tag') {
    throw new Error(`El tag '${tag}' debe ser anotado (git tag -a), no lightweight (R-CD-1).`);
  }

  const sha = git(root, ['rev-list', '-n1', tagRef]);
  if (git(root, ['rev-parse', '--verify', mainRef], { allowFailure: true }) === null) {
    throw new Error(`No existe la referencia '${mainRef}'.`);
  }
  if (git(root, ['merge-base', '--is-ancestor', sha, mainRef], { allowFailure: true }) === null) {
    throw new Error(`El commit ${sha} del tag '${tag}' no está en '${mainRef}' (R-CD-2).`);
  }

  const mergedTags = git(root, ['tag', '--merged', mainRef, '--list', 'v*.*.*'])
    .split('\n')
    .filter(Boolean);
  const previous = mergedTags
    .filter((candidate) => candidate !== tag)
    .map((candidate) => ({ tag: candidate, parsed: parseTag(candidate) }))
    .filter((entry) => entry.parsed)
    .sort((a, b) => compareSemver(a.parsed, b.parsed));
  const latest = previous.at(-1);

  if (latest && compareSemver(parsed, latest.parsed) <= 0) {
    throw new Error(
      `El tag '${tag}' no es mayor que el último release '${latest.tag}' (R-CD-1).`,
    );
  }

  const versionFrom = parseTag(VERSION_MATCH_FROM);
  const baseVersion = { ...parsed, prerelease: [] };
  const baseFrom = { ...versionFrom, prerelease: [] };
  let checkedFiles = 0;
  if (compareSemver(baseVersion, baseFrom) >= 0) {
    checkedFiles = checkVersionMatch(root, sha, tag);
  }

  if (checkRemote) {
    // El wildcard incluye la línea `^{}` con el commit pelado del tag anotado;
    // con la ref exacta, ls-remote solo devuelve el objeto tag.
    const remote = git(root, ['ls-remote', '--tags', 'origin', `${tagRef}*`], {
      allowFailure: true,
    });
    if (remote) {
      const lines = remote.split('\n').filter(Boolean);
      const peeled = lines.find((line) => line.endsWith('^{}')) ?? lines[0];
      const remoteSha = peeled.split(/\s+/)[0];
      if (remoteSha !== sha) {
        throw new Error(
          `El tag '${tag}' ya existe en origin apuntando a ${remoteSha}; un tag publicado no se mueve (R-CD-1).`,
        );
      }
    }
  }

  console.log(`✅ Release '${tag}' válida.`);
  console.log(`   commit: ${sha}`);
  console.log(`   main:   ${mainRef}`);
  console.log(`   anterior: ${latest?.tag ?? '(ninguno)'}`);
  if (checkedFiles > 0) {
    console.log(`   versiones: ${tag.slice(1)} en ${checkedFiles} package.json + lockfile`);
  } else {
    console.log(`   versiones: omitidas (${tag} < ${VERSION_MATCH_FROM})`);
  }
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
