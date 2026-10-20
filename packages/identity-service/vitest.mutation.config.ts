import { defineConfig } from 'vitest/config';
import swc from 'unplugin-swc';
import { resolve } from 'node:path';

// Stryker ejecuta desde `.stryker-tmp/sandbox-*`; resolvemos la raíz real para
// que el alias de shared apunte al fuente y no a una copia inexistente.
const repoRoot = process.cwd().includes('.stryker-tmp')
  ? resolve(process.cwd(), '../../../..')
  : resolve(process.cwd(), '../..');

export default defineConfig({
  plugins: [swc.vite()],
  resolve: {
    alias: {
      '@core/shared': resolve(repoRoot, 'packages/shared/src/index.ts'),
    },
  },
  test: {
    globals: true,
    environment: 'node',
    include: ['src/**/*.spec.ts'],
    fileParallelism: false,
  },
});
