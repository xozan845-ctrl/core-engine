/**
 * Semver mínimo (sin dependencias) para validar tags de release.
 *
 * Soporta `v<major>.<minor>.<patch>[-<prerelease>]`, con precedencia estándar:
 * un prerelease siempre es menor que la misma versión estable y los
 * identificadores numéricos se comparan como números (no como texto).
 */

const TAG_PATTERN = /^v(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/;

/**
 * @param {string} tag
 * @returns {{ raw: string, major: number, minor: number, patch: number, prerelease: string[] } | null}
 */
export function parseTag(tag) {
  const match = TAG_PATTERN.exec(tag ?? '');
  if (!match) return null;

  return {
    raw: tag,
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4] ? match[4].split('.') : [],
  };
}

/**
 * Compara dos versiones parseadas. Devuelve -1, 0 o 1.
 *
 * @param {ReturnType<typeof parseTag>} a
 * @param {ReturnType<typeof parseTag>} b
 * @returns {number}
 */
export function compareSemver(a, b) {
  for (const part of ['major', 'minor', 'patch']) {
    if (a[part] !== b[part]) return a[part] < b[part] ? -1 : 1;
  }

  return comparePrerelease(a.prerelease, b.prerelease);
}

function comparePrerelease(a, b) {
  if (a.length === 0 && b.length === 0) return 0;
  if (a.length === 0) return 1;
  if (b.length === 0) return -1;

  const length = Math.max(a.length, b.length);
  for (let i = 0; i < length; i += 1) {
    const left = a[i];
    const right = b[i];

    if (left === undefined) return -1;
    if (right === undefined) return 1;

    const leftIsNumber = /^\d+$/.test(left);
    const rightIsNumber = /^\d+$/.test(right);

    if (leftIsNumber && rightIsNumber) {
      const diff = Number(left) - Number(right);
      if (diff !== 0) return diff < 0 ? -1 : 1;
      continue;
    }

    if (leftIsNumber) return -1;
    if (rightIsNumber) return 1;
    if (left !== right) return left < right ? -1 : 1;
  }

  return 0;
}
