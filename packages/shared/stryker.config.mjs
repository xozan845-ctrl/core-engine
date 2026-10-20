import { strykerBase } from '../../scripts/config/stryker.base.mjs';

export default {
  ...strykerBase,
  vitest: { configFile: 'vitest.mutation.config.ts' },
  mutate: ['src/money.ts', 'src/order-state.ts'],
};
