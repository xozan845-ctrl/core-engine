import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * Higiene de secretos (R-DS-1 / R-CD-7).
 *
 * La plantilla de desarrollo solo contiene valores identificables como locales,
 * el runtime no arranca en producción sin secretos y el seed no siembra un
 * administrador con contraseña fija en el código.
 */

const ENV = readText(join(repoRoot, '.env.example'));
const RUNTIME = readText(join(repoRoot, 'packages/shared/src/config/runtime.ts'));
const SEED = readText(join(repoRoot, 'packages/identity-service/src/seed.rs'));
const RUST_CONFIG = readText(join(repoRoot, 'packages/identity-service/src/config.rs'));

// Secretos de despliegue que ya se filtraron una vez; no deben volver al repo.
const SECRETOS_REALES = ['846eb67047faf8686b561c5c5d096cbb7b2f980e92d6e205'];

const CLAVES_MARCADAS = ['JWT_SECRET', 'INTERNAL_API_KEY'];
const CLAVES_RUNTIME = ['JWT_SECRET', 'ADMIN_PASSWORD', 'ADMIN_EMAIL'];

export default defineSuite({
  id: 'security-secrets-hygiene',
  area: 'security',
  rules: ['R-DS-1', 'R-CD-7'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-1: la plantilla no contiene secretos reales', () => {
      for (const valor of SECRETOS_REALES) {
        assert.ok(!ENV.includes(valor), `.env.example contiene el secreto "${valor}"`);
      }
      for (const clave of CLAVES_MARCADAS) {
        const linea = ENV.split('\n').find((l) => l.startsWith(`${clave}=`));
        assert.ok(linea, `falta ${clave} en .env.example`);
        assert.match(
          linea,
          /(cambiar_en_produccion|<GENERAR)/,
          `${clave} debe marcarse como valor de desarrollo o placeholder`,
        );
      }
      const admin = ENV.split('\n').find((l) => l.startsWith('ADMIN_PASSWORD='));
      assert.ok(admin, 'falta ADMIN_PASSWORD en .env.example');
    });

    test('R-DS-1: producción falla cerrado si falta un secreto', () => {
      assert.match(
        RUNTIME,
        /(esProduccion|NODE_ENV)/,
        'runtime no distingue producción',
      );
      assert.match(RUNTIME, /throw new Error\(/, 'runtime no detiene el arranque sin secretos');
      for (const clave of CLAVES_RUNTIME) {
        assert.ok(RUNTIME.includes(clave), `runtime no exige ${clave}`);
      }
    });

    test('R-DS-1: el seed no conserva una contraseña por defecto', () => {
      assert.ok(
        !SEED.includes('AdminCore Engine2026!'),
        'el seed conserva la contraseña por defecto',
      );
      assert.ok(
        SEED.includes('admin_password'),
        'el seed debe obtener la contraseña del runtime',
      );
      for (const clave of CLAVES_RUNTIME) {
        assert.ok(RUST_CONFIG.includes(clave), `config.rs no exige ${clave}`);
      }
      assert.match(RUST_CONFIG, /produccion/, 'config.rs no distingue producción');
    });
  },
});
