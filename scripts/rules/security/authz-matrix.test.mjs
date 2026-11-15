import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * Autorización del borde (R-GW-1 / R-GW-3 / R-DS-5).
 *
 * Verifica que el gateway mantenga el deny-by-default, que las rutas públicas
 * estén declaradas explícitamente, que las protegidas tengan roles y que las
 * cabeceras de identidad/internas se limpien antes de reenviar.
 */

const POLITICAS = readText(join(repoRoot, 'packages/api-gateway/src/policy.rs'));
const AUTH = readText(join(repoRoot, 'packages/api-gateway/src/auth.rs'));
const PROXY = readText(join(repoRoot, 'packages/api-gateway/src/proxy.rs'));

const lineas = POLITICAS.split('\n');
const rutaCon = (ruta, fragmento) =>
  lineas.some(
    (linea) =>
      linea.includes(`/api/v2/${ruta}`) && linea.includes(fragmento),
  );

export default defineSuite({
  id: 'security-authz-matrix',
  area: 'security',
  rules: ['R-GW-1', 'R-GW-3', 'R-DS-5'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-GW-1: el gateway conserva el deny-by-default para /api/v2', () => {
      assert.ok(
        POLITICAS.includes('p(r"^/api/v2/", &["*"], Some(Vec::new()))'),
        'falta la política deny-by-default /^/api/v2/ con roles vacíos',
      );
    });

    test('R-GW-1: las rutas públicas están declaradas explícitamente', () => {
      for (const ruta of [
        'auth/login',
        'auth/registro',
        'auth/refresh',
        'auth/restablecer-contrasena',
        'auth/restablecer-contrasena/confirmar',
        'configuracion/sitio',
        'tiendas/[^/]+$',
      ]) {
        assert.ok(
          rutaCon(ruta, 'None'),
          `${ruta} no está declarada como pública (None)`,
        );
      }
    });

    test('R-GW-1/R-GW-3: las rutas sensibles exigen roles', () => {
      assert.ok(rutaCon('orders', 'Some(vec!['), 'orders no tiene una política de roles');
      for (const ruta of ['usuarios', 'finanzas', 'admin', 'storage']) {
        assert.ok(
          POLITICAS.includes(`/api/v2/${ruta}`) && POLITICAS.includes('Some(vec!['),
          `${ruta} no tiene una política de roles`,
        );
      }
    });

    test('R-GW-3/R-DS-5: el gateway limpia identidad, tenant y clave interna', () => {
      for (const cabecera of ['"x-user-id"', '"x-user-rol"', '"x-user-email"', 'TENANT_HEADER']) {
        assert.ok(AUTH.includes(cabecera), `la identidad autenticada no define ${cabecera}`);
      }
      assert.ok(
        AUTH.includes('salida.insert("x-user-id"'),
        'la identidad debe construirse desde el token verificado',
      );
      assert.ok(
        PROXY.includes('for (clave, valor) in auth'),
        'el proxy solo debe reenviar las cabeceras de contexto autenticadas',
      );
      assert.ok(
        !PROXY.includes('"x-internal-key"'),
        'la clave interna del cliente no debe reenviarse al destino',
      );
    });

    test('R-GW-1: el gateway no enruta /internal', () => {
      assert.ok(!PROXY.includes('/internal'), 'el gateway no debe tener destinos /internal');
    });
  },
});
