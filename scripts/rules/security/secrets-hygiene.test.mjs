import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * Higiene de secretos (R-DS-1 / R-CD-7).
 *
 * La plantilla de desarrollo no puede contener secretos reales, el código no
 * puede volver al fallback `dev_secret` ni sembrar un admin con contraseña por
 * defecto, y el generador local debe producir valores aleatorios.
 */

const ENV = readText(join(repoRoot, '.env.example'));
const SECRETOS = readText(join(repoRoot, 'packages/shared/src/config/secretos.ts'));
const SEED = readText(join(repoRoot, 'packages/identity-service/src/seed/seed.service.ts'));
const DEV_ENV = readText(join(repoRoot, 'scripts/tools/dev-env.mjs'));

const PROHIBIDOS = [
  'dev_secret_cambiar_en_produccion',
  'dev_internal_key_cambiar_en_produccion',
  'AdminCore Engine2026!',
  'core_engine_dev',
  '0b66c112e3ceca4e3b6a648219c88c60',
];

const CLAVES = ['JWT_SECRET', 'INTERNAL_API_KEY', 'ADMIN_PASSWORD'];

export default defineSuite({
  id: 'security-secrets-hygiene',
  area: 'security',
  rules: ['R-DS-1', 'R-CD-7'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-1: la plantilla no contiene secretos reales', () => {
      for (const valor of PROHIBIDOS) {
        assert.ok(!ENV.includes(valor), `.env.example contiene el secreto "${valor}"`);
      }
      for (const clave of CLAVES) {
        const linea = ENV.split('\n').find((l) => l.startsWith(`${clave}=`));
        assert.ok(linea, `falta ${clave} en .env.example`);
        assert.match(linea, /<GENERAR/, `${clave} debe ser un placeholder, no un secreto`);
      }
    });

    test('R-DS-1: el código no tiene fallback dev_secret ni admin por defecto', () => {
      assert.ok(!SECRETOS.includes("?? 'dev_secret'"), 'secretos.ts conserva el fallback dev_secret');
      assert.match(
        SECRETOS,
        /(cambiar_en_produccion|generar_con_npm)/,
        'la lista de placeholders no cubre los valores históricos',
      );
      assert.ok(!SEED.includes('AdminCore Engine2026!'), 'el seed conserva la contraseña por defecto');
      assert.ok(SEED.includes('ADMIN_PASSWORD'), 'el seed debe leer ADMIN_PASSWORD del entorno');
    });

    test('R-CD-7: el generador local produce secretos aleatorios', () => {
      assert.ok(DEV_ENV.includes('randomBytes'), 'dev-env no genera valores aleatorios');
      for (const clave of CLAVES) {
        assert.ok(DEV_ENV.includes(clave), `dev-env no rota ${clave}`);
      }
    });
  },
});
