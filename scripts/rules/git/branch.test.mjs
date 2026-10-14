import { defineSuite } from '../../engine/suite.mjs';
import { getBranchContext, getCommitsAgainstBase } from '../../engine/lib/git.mjs';
import { validateCommitSubject } from '../../engine/lib/commits.mjs';
import {
  ALLOWED_COMMIT_SCOPES,
  ALLOWED_COMMIT_TYPES,
  INTEGRATION_BRANCH,
  MAIN_BRANCH,
} from '../../config/project.mjs';

/**
 * Regla R-GC-1..4 (parte contextual): los commits que introduce el PR deben
 * seguir Conventional Commits válidos para este repo. Inspecciona git real.
 */

const allowed = {
  types: ALLOWED_COMMIT_TYPES,
  scopes: ALLOWED_COMMIT_SCOPES,
};

export default defineSuite({
  id: 'git-branch',
  area: 'git',
  rules: ['R-GC-1', 'R-GC-2', 'R-GC-3', 'R-GC-4'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-GC-1..4: los commits introducidos por el PR cumplen el convenio del repo', () => {
      const { currentBranch, baseBranch } = getBranchContext(
        MAIN_BRANCH,
        INTEGRATION_BRANCH,
      );
      if (currentBranch === MAIN_BRANCH || currentBranch === `origin/${MAIN_BRANCH}`) {
        return; // en main no hay commits que evaluar
      }

      const commits = getCommitsAgainstBase(baseBranch, MAIN_BRANCH);
      if (commits.length === 0) return;

      const problems = [];
      for (const { subject } of commits) {
        const errors = validateCommitSubject(subject, allowed);
        if (errors.length > 0) {
          problems.push(`- "${subject}"\n    ${errors.join('\n    ')}`);
        }
      }

      assert.deepEqual(
        problems,
        [],
        `Commits no conformes a Conventional Commits (R-GC-1..4):\n${problems.join('\n')}`,
      );
    });
  },
});
