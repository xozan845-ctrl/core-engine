import { defineSuite } from '../../engine/suite.mjs';
import { validateCommitSubject } from '../../engine/lib/commits.mjs';
import {
  ALLOWED_COMMIT_SCOPES,
  ALLOWED_COMMIT_TYPES,
} from '../../config/project.mjs';

/**
 * Regla R-GC-1..4 (parte pura): `validateCommitSubject` se prueba con datos
 * sintéticos, sin tocar el repositorio ni git. La parte contextual (los commits
 * reales del PR) vive en `branch.test.mjs`, en modo `repo`.
 */

const allowed = {
  types: ALLOWED_COMMIT_TYPES,
  scopes: ALLOWED_COMMIT_SCOPES,
};

export default defineSuite({
  id: 'git-commits',
  area: 'git',
  rules: ['R-GC-1', 'R-GC-2', 'R-GC-3', 'R-GC-4'],
  mode: 'pure',
  run: ({ test, assert }) => {
    test('commits (unit): acepta asuntos conformes', () => {
      for (const subject of [
        'ci(rules): validar los commits del PR (R-GC-1)',
        'feat(orders): añadir transición de estado (R-DI-3)',
        'fix(gateway): rechazar el header sin token',
        'docs(rules): corregir enlaces internos',
        'refactor(shared): extraer el cliente de eventos',
        'build(deps): subir NestJS a la última minor',
      ]) {
        assert.deepEqual(
          validateCommitSubject(subject, allowed),
          [],
          `debería aceptar: ${subject}`,
        );
      }
    });

    test('commits (unit): rechaza formato, tipo y ámbito inválidos', () => {
      assert.match(validateCommitSubject('wip', allowed)[0], /formato/);
      assert.match(validateCommitSubject('fix', allowed)[0], /formato/);
      assert.match(
        validateCommitSubject('feat: sin ámbito', allowed).join(' '),
        /falta el ámbito/,
      );
      assert.match(
        validateCommitSubject('feat(zzz): algo', allowed).join(' '),
        /ámbito 'zzz' no reconocido/,
      );
    });

    test('commits (unit): rechaza gerundio, pasado y resumen genérico', () => {
      assert.match(
        validateCommitSubject('feat(orders): agregando endpoint', allowed).join(' '),
        /gerundio/,
      );
      assert.match(
        validateCommitSubject('fix(gateway): corregí el azp', allowed).join(' '),
        /pasado/,
      );
      assert.match(
        validateCommitSubject('feat(orders): wip', allowed).join(' '),
        /genérico/,
      );
    });
  },
});
