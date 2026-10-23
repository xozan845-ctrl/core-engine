/**
 * Lectura y comparación de los reportes `coverage-summary.json` (R-COV-1).
 *
 * El baseline vive en `scripts/config/coverage-baseline.json` y se actualiza
 * explícitamente con `npm run coverage:baseline`. El gate de CI compara el
 * reporte recién generado contra ese baseline: ninguna métrica puede bajar.
 */
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const COVERAGE_METRICS = ['lines', 'branches', 'functions', 'statements'];

/**
 * @param {string} root Raíz del repositorio.
 * @returns {Record<string, Record<string, number>>}
 */
export function readCoverageSummaries(root) {
  const packagesDir = join(root, 'packages');
  if (!existsSync(packagesDir)) {
    throw new Error(`No existe ${packagesDir}; ejecuta el repo desde su raíz.`);
  }

  const result = {};
  for (const entry of readdirSync(packagesDir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const file = join(packagesDir, entry.name, 'coverage', 'coverage-summary.json');
    if (!existsSync(file)) continue;

    const total = JSON.parse(readFileSync(file, 'utf8')).total;
    result[entry.name] = Object.fromEntries(
      COVERAGE_METRICS.map((metric) => [metric, total?.[metric]?.pct]),
    );
  }

  return Object.fromEntries(Object.entries(result).sort(([a], [b]) => a.localeCompare(b)));
}

/**
 * Compara la cobertura actual con el baseline. Devuelve una lista de problemas
 * (vacía si ninguna métrica bajó y ambos conjuntos de workspaces coinciden).
 *
 * @param {Record<string, Record<string, number>>} current
 * @param {Record<string, Record<string, number>>} baseline
 * @returns {string[]}
 */
export function compareCoverage(current, baseline) {
  const problems = [];

  for (const [workspace, expected] of Object.entries(baseline)) {
    const actual = current[workspace];
    if (!actual) {
      problems.push(`${workspace}: sin reporte de cobertura (¿faltó correr sus tests?)`);
      continue;
    }

    for (const metric of COVERAGE_METRICS) {
      const expectedValue = expected?.[metric];
      const actualValue = actual?.[metric];

      if (typeof expectedValue !== 'number') {
        problems.push(`${workspace}: baseline sin la métrica '${metric}'`);
        continue;
      }
      if (typeof actualValue !== 'number') {
        problems.push(`${workspace}: reporte sin la métrica '${metric}'`);
        continue;
      }
      if (actualValue + 1e-9 < expectedValue) {
        problems.push(
          `${workspace}: ${metric} ${actualValue}% < baseline ${expectedValue}% (${(actualValue - expectedValue).toFixed(2)})`,
        );
      }
    }
  }

  for (const workspace of Object.keys(current)) {
    if (!baseline[workspace]) {
      problems.push(`${workspace}: sin baseline; ejecuta npm run coverage:baseline`);
    }
  }

  return problems;
}
