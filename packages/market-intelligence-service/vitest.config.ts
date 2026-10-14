import { defineConfig } from 'vitest/config';
import swc from 'unplugin-swc';
import { resolve } from 'node:path';

// Config de pruebas del paquete (Vitest). Reproduce la config de Jest que vivía
// en package.json: specs `*.spec.ts`, entorno node, alias `@core/shared` y
// umbrales de cobertura (R-QA-3/R-QA-4).
export default defineConfig({
  // NestJS necesita metadatos de decoradores para la DI: esbuild (por defecto en
  // Vite/Vitest) no los emite, así que se compila con SWC.
  plugins: [swc.vite()],
  resolve: {
    alias: {
      '@core/shared': resolve(__dirname, '../shared/src/index.ts'),
    },
  },
  test: {
    globals: true,
    environment: 'node',
    include: ['src/**/*.spec.ts'],
    fileParallelism: false,
    coverage: {
      provider: 'v8',
      include: ['src/**/*.ts'],
      exclude: [
        'src/main.ts',
        'src/**/*.module.ts',
        'src/**/*.spec.ts',
        'src/swagger.ts',
        'src/swagger-export.ts',
      ],
      reporter: ['text', 'lcov', 'json-summary'],
      reportsDirectory: './coverage',
      reportOnFailure: true,
      thresholds: { lines: 18, branches: 11 },
    },
  },
});
