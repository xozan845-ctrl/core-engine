import { strykerBase } from '../../scripts/config/stryker.base.mjs';

export default {
  ...strykerBase,
  vitest: { configFile: 'vitest.mutation.config.ts' },
  thresholds: { ...strykerBase.thresholds, break: 90 },
  mutate: ['src/contabilidad/contabilidad.service.ts'],
};
