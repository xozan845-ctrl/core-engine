import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readJson } from '../../engine/lib/fs.mjs';
import { checkRlsCoverage } from '../../engine/lib/db.mjs';

/**
 * R-SB-2: toda tabla transaccional debe tener RLS y políticas. El baseline
 * enumera la deuda existente para impedir que crezca; una tabla nueva sin RLS
 * o una excepción obsoleta falla el gate.
 */
export default defineSuite({
  id: 'database-rls',
  area: 'db',
  rules: ['R-SB-2'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-SB-2: las tablas sin RLS son solo las del baseline', () => {
      const baseline = readJson(
        join(repoRoot, 'scripts', 'config', 'rls-baseline.json'),
      );
      const problems = checkRlsCoverage(repoRoot, baseline);

      assert.deepEqual(
        problems,
        [],
        `Cobertura RLS fuera del baseline (R-SB-2):\n${problems.join('\n')}`,
      );
    });
  },
});
