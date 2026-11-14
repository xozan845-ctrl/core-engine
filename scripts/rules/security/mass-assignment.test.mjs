import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * Mass assignment / validación estricta (R-DS-4).
 *
 * Los servicios Rust validan el cuerpo con esquemas explícitos por handler y
 * descartan cualquier campo no declarado. Este check fija el contrato mínimo:
 * validación declarativa y errores estructurados, no binding directo.
 */

const VALIDACION = join(
  repoRoot,
  'packages/identity-service/src/http/validacion.rs',
);

export default defineSuite({
  id: 'security-mass-assignment',
  area: 'security',
  rules: ['R-DS-4'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-4: los DTOs Rust se validan contra un esquema explícito', () => {
      assert.ok(existsSync(VALIDACION), 'falta la validación de DTOs de identity');
      const codigo = readText(VALIDACION);
      assert.match(codigo, /pub struct ReglaCampo/, 'falta el contrato de campo');
      assert.match(codigo, /pub fn validar\(/, 'falta la función de validación');
      assert.match(codigo, /"restricciones"/, 'los errores deben detallar restricciones');
      assert.match(codigo, /ApiError::validacion/, 'la validación debe devolver 400 VALIDACION');
    });
  },
});
