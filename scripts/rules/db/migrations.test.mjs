import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readJson } from '../../engine/lib/fs.mjs';
import { checkMigrationManifest } from '../../engine/lib/db.mjs';

/**
 * R-DB-6: las migraciones de `infra/db/init/*.sql` son la fuente canónica y no
 * se editan una vez registradas. El manifiesto de hashes hace visible cualquier
 * cambio: una migración nueva se registra, una aplicada no se toca.
 */
export default defineSuite({
  id: 'database-migrations',
  area: 'db',
  rules: ['R-DB-6'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DB-6: las migraciones init están íntegras y registradas', () => {
      const manifest = readJson(
        join(repoRoot, 'scripts', 'config', 'db-migrations.json'),
      );
      const problems = checkMigrationManifest(repoRoot, manifest);

      assert.deepEqual(
        problems,
        [],
        `Migraciones fuera del manifiesto (R-DB-6):\n${problems.join('\n')}`,
      );
    });
  },
});
