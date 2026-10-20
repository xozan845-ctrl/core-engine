/**
 * Base de configuración de Stryker (R-MT-1/R-MT-2).
 *
 * Los módulos críticos se someten a mutación mensual o pre-release, nunca en el
 * gate de cada PR. Se usa el runner `command` (Vitest real sobre el sandbox)
 * porque el runner de Vitest no aplicaba el mutation switching a `money.ts`.
 * `break` se activa por paquete cuando su baseline supera el 90 % (R-MT-2).
 */
export const strykerBase = {
  testRunner: 'command',
  commandRunner: { command: 'npx vitest run --config vitest.mutation.config.ts' },
  plugins: ['@stryker-mutator/vitest-runner'],
  coverageAnalysis: 'off',
  reporters: ['clear-text', 'progress', 'json'],
  jsonReporter: { fileName: 'reports/mutation/mutation.json' },
  // Los paquetes sin baseline estabilizado corren en modo reporte; cada uno
  // activa `break: 90` cuando alcanza el mínimo de R-MT-2.
  thresholds: { high: 90, low: 70, break: null },
  timeoutMS: 30_000,
  concurrency: 2,
  cleanTempDir: true,
};
