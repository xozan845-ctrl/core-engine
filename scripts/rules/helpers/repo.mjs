import { readdirSync, readFileSync } from 'node:fs';
import { execFileSync, execSync } from 'node:child_process';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { MAIN_BRANCH } from '../../project.config.mjs';

export const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
export const rulesRoot = join(repositoryRoot, 'docs', 'rules');
export const workflowsDirectory = join(repositoryRoot, '.github', 'workflows');

/** Recorre recursivamente todos los `.md` de un directorio (saltando ruido). */
export function walkMarkdown(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (
      entry.name === 'node_modules' ||
      entry.name === '.git' ||
      entry.name === '.agents' ||
      entry.name === 'dist' ||
      entry.name === 'coverage'
    ) {
      return [];
    }
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return walkMarkdown(path);
    return entry.isFile() && entry.name.endsWith('.md') ? [path] : [];
  });
}

/** Áreas de `docs/rules/` (carpetas), excluyendo `_meta`. */
export function getRuleAreas() {
  return readdirSync(rulesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name !== '_meta')
    .map((entry) => entry.name)
    .sort();
}

/** Archivos de reglas `NN-tema.md` de todas las áreas. */
export function getRuleFiles() {
  return getRuleAreas().flatMap((area) =>
    readdirSync(join(rulesRoot, area), { withFileTypes: true })
      .filter((file) => file.isFile() && /^\d{2}-.+\.md$/.test(file.name))
      .map((file) => join(rulesRoot, area, file.name)),
  );
}

export function relativePath(path) {
  return relative(repositoryRoot, path);
}

export function getBranchContext() {
  const envHead = process.env.GITHUB_HEAD_REF;
  const envBase = process.env.GITHUB_BASE_REF;
  let currentBranch = envHead || '';

  if (!currentBranch) {
    try {
      currentBranch = execSync('git rev-parse --abbrev-ref HEAD', {
        cwd: repositoryRoot,
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'ignore'],
      }).trim();
    } catch {
      currentBranch = 'unknown';
    }
  }

  const baseBranch = envBase ? `origin/${envBase}` : `origin/${MAIN_BRANCH}`;
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

/** Commits sin merges entre la rama base y HEAD (sujeto + cuerpo). */
export function getCommitsAgainstBase(baseBranch = `origin/${MAIN_BRANCH}`) {
  const tryLog = (target) => {
    try {
      const out = execFileSync(
        'git',
        ['log', `${target}..HEAD`, '--no-merges', '--format=%s%x00%b%x1e'],
        { cwd: repositoryRoot, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] },
      );
      return parseGitLog(out);
    } catch {
      return null;
    }
  };

  return tryLog(baseBranch) ?? tryLog(MAIN_BRANCH) ?? [];
}

export function isDocumentationExempt(commitLog) {
  if (process.env.CHECK_RULES_SKIP_DOC === '1') return true;
  if (!commitLog) return false;
  return /\[(?:skip-doc|no-doc)\]|R-DO-8:\s*(?:skip|exento|sin cambios)/i.test(commitLog);
}

/** Lee el contenido de un archivo de una ref de git, o `null` si no existe. */
export function gitShow(ref, repoRelativePath) {
  try {
    return execSync(`git show ${ref}:${repoRelativePath}`, {
      cwd: repositoryRoot,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch {
    return null;
  }
}
