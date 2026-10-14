/**
 * Envoltura de git sin interpolación de shell: cada argumento viaja como argv de
 * `execFileSync`, nunca concatenado en un comando (R-DS: nada de inyección por
 * construcción de strings).
 */
import { execFileSync } from 'node:child_process';
import { repoRoot } from './paths.mjs';

function git(args, { allowFailure = false } = {}) {
  try {
    return execFileSync('git', args, {
      cwd: repoRoot,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch (error) {
    if (allowFailure) return null;
    throw error;
  }
}

/**
 * Resuelve la base de comparación local. En CI, la rama base declarada por el
 * evento del PR siempre tiene prioridad.
 */
export function resolveBaseBranch({
  currentBranch,
  envBase,
  mainBranch,
  integrationBranch,
}) {
  if (envBase) return `origin/${envBase}`;

  const localBase =
    currentBranch === mainBranch || currentBranch.startsWith('hotfix/')
      ? mainBranch
      : integrationBranch;

  return `origin/${localBase}`;
}

/** Rama actual y rama base, con soporte para el entorno de GitHub Actions. */
export function getBranchContext(mainBranch, integrationBranch = mainBranch) {
  const envHead = process.env.GITHUB_HEAD_REF;
  const envBase = process.env.GITHUB_BASE_REF;
  let currentBranch = envHead || '';

  if (!currentBranch) {
    const out = git(['rev-parse', '--abbrev-ref', 'HEAD'], { allowFailure: true });
    currentBranch = (out ?? 'unknown').trim();
  }

  const baseBranch = resolveBaseBranch({
    currentBranch,
    envBase,
    mainBranch,
    integrationBranch,
  });
  return { currentBranch, baseBranch };
}

/**
 * Parsea la salida de `git log --format=%s%x00%b%x1e`: cada commit es
 * `<sujeto>\0<cuerpo><RS>`.
 */
export function parseGitLog(raw) {
  if (!raw) return [];
  return raw
    .split('\u001e')
    .map((chunk) => chunk.replace(/^\n/, '').trimEnd())
    .filter((chunk) => chunk.length > 0)
    .map((chunk) => {
      const [subject = '', body = ''] = chunk.split('\u0000');
      return { subject: subject.trim(), body: body.trim() };
    });
}

/** Commits sin merges entre `baseBranch` y HEAD; si falla, prueba `mainBranch`. */
export function getCommitsAgainstBase(baseBranch, mainBranch) {
  const tryLog = (target) => {
    const out = git(
      ['log', `${target}..HEAD`, '--no-merges', '--format=%s%x00%b%x1e'],
      { allowFailure: true },
    );
    return out === null ? null : parseGitLog(out);
  };

  return tryLog(baseBranch) ?? tryLog(mainBranch) ?? [];
}

/** Contenido de un archivo en una ref de git, o `null` si no existe. */
export function gitShow(ref, repoRelativePath) {
  return git(['show', `${ref}:${repoRelativePath}`], { allowFailure: true });
}
