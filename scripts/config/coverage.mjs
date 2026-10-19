/**
 * config/coverage.mjs — Core Engine
 * ──────────────────────────────────────────────────────────────────────────────
 * Fuente única de los umbrales de cobertura por workspace (R-COV-1, R-COV-4).
 *
 * Cada `packages/<workspace>/vitest.config.ts` consume `coverageThresholdsFor()`; no
 * repite porcentajes. Así el ratchet (+5 por release, nunca a la baja) se edita
 * en un solo lugar; el baseline documentado vive en
 * `docs/rules/test/06-estandares-cobertura.md`.
 *
 * `shared` es librería; el resto son servicios desplegables.
 * ──────────────────────────────────────────────────────────────────────────────
 */

export const COVERAGE_THRESHOLDS = {
  shared: { lines: 39, branches: 39, functions: 55 },
  'api-gateway': { lines: 50, branches: 51, functions: 57 },
  'catalog-service': { lines: 93, branches: 82, functions: 90 },
  'commissions-service': { lines: 20, branches: 14, functions: 22 },
  'field-service': { lines: 84, branches: 70, functions: 72 },
  'finance-service': { lines: 70, branches: 52, functions: 72 },
  'identity-service': { lines: 89, branches: 78, functions: 85 },
  'logistics-service': { lines: 99, branches: 96, functions: 99 },
  'market-intelligence-service': { lines: 18, branches: 11, functions: 35 },
  'orders-service': { lines: 32, branches: 23, functions: 33 },
  'storage-service': { lines: 99, branches: 86, functions: 99 },
  'stores-service': { lines: 86, branches: 84, functions: 76 },
};

/**
 * Devuelve los umbrales del workspace o falla si no está declarado. Falla
 * ruidosamente para que un servicio nuevo no quede sin gate por omisión.
 *
 * @param {string} workspace Nombre de la carpeta en `packages/`.
 * @returns {{ lines: number, branches: number, functions: number }}
 */
export function coverageThresholdsFor(workspace) {
  const thresholds = COVERAGE_THRESHOLDS[workspace];
  if (!thresholds) {
    throw new Error(
      `Workspace sin umbrales de cobertura en scripts/config/coverage.mjs: ${workspace}`,
    );
  }
  return thresholds;
}
