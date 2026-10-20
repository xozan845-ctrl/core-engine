import { strykerBase } from '../../scripts/config/stryker.base.mjs';

export default {
  ...strykerBase,
  vitest: { configFile: 'vitest.mutation.config.ts' },
  mutate: ['src/auth/**/*.ts', 'src/usuarios/**/*.ts'],
};
