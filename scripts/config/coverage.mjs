/**
 * config/coverage.mjs — Core Engine
 *
 * Ya no hay workspaces Node con suite Vitest: todos los servicios están en
 * Rust y su gate vive en `cargo llvm-cov` (jobs `rust` de CI). Este módulo se
 * conserva vacío para las reglas de gobernanza que lo importan y para dejar
 * explícito que no queda cobertura Node que comparar.
 */

export const COVERAGE_THRESHOLDS = {};

export function coverageThresholdsFor(workspace) {
  const thresholds = COVERAGE_THRESHOLDS[workspace];
  if (!thresholds) {
    throw new Error(
      `Workspace sin umbrales de cobertura en scripts/config/coverage.mjs: ${workspace}`,
    );
  }
  return thresholds;
}
