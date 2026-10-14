import { defineConfig } from 'vitest/config';
import swc from 'unplugin-swc';
import { resolve } from 'node:path';

export default defineConfig({
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
      thresholds: { lines: 84, branches: 70 },
    },
  },
});
