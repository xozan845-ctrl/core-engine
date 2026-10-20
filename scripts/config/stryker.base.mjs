/**
 * Base de configuración de Stryker (R-MT-1/R-MT-2).
 *
 * Los módulos críticos se someten a mutación mensual o pre-release, nunca en el
 * gate de cada PR. `break: 90` aplica el mínimo de R-MT-2; por debajo del 70 %
 * el release queda bloqueado. Cada paquete declara su propio `mutate`.
 */
export const strykerBase = {
  testRunner: 'vitest',
  plugins: ['@stryker-mutator/vitest-runner'],
  coverageAnalysis: 'all',
  reporters: ['clear-text', 'progress', 'json'],
  jsonReporter: { fileName: 'reports/mutation/mutation.json' },
  // Primera ronda: reporte sin bloquear. Al estabilizar el baseline se activa
  // `break: 90` (R-MT-2) y el workflow mensual pasa a ser gate de release.
  thresholds: { high: 90, low: 70, break: null },
  timeoutMS: 30_000,
  concurrency: 2,
  cleanTempDir: true,
};
