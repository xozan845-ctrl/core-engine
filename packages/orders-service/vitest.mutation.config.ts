import { defineConfig } from 'vitest/config';
import swc from 'unplugin-swc';
import { resolve } from 'node:path';

// Config mínima para Stryker: sin coverage thresholds ni imports fuera del paquete.
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
  },
});
