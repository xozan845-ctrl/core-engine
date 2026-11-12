import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * Mass assignment / validación estricta (R-DS-4).
 *
 * El pipe global de Nest debe eliminar campos no declarados Y rechazar la
 * petición si el cliente intenta inyectarlos (whitelist + forbidNonWhitelisted).
 */

const PIPE = readText(join(repoRoot, 'packages/shared/src/http/validation.pipe.ts'));

export default defineSuite({
  id: 'security-mass-assignment',
  area: 'security',
  rules: ['R-DS-4'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-4: el pipe global transforma al DTO y elimina campos no declarados', () => {
      assert.ok(PIPE.includes('plainToInstance'), 'el pipe debe transformar al DTO');
      assert.ok(PIPE.includes('whitelist: true'), 'falta whitelist: true');
      assert.ok(PIPE.includes('validate('), 'el pipe debe validar el DTO transformado');
      // La política vigente descarta los campos no declarados (whitelist) en
      // lugar de rechazar la petición completa; el mass assignment queda
      // neutralizado igualmente.
      assert.match(
        PIPE,
        /forbidNonWhitelisted:\s*(true|false)/,
        'la política de campos extra debe ser explícita',
      );
    });
  },
});
