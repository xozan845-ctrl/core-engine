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

const AUTH = readText(
  join(repoRoot, 'packages/api-gateway/src/proxy/gateway-auth.middleware.ts'),
);
const PROXY = readText(join(repoRoot, 'packages/api-gateway/src/proxy/proxy.service.ts'));

const lineas = AUTH.split('\n');
// Los patrones son regex literales (`\/`), se normalizan para comparar por ruta.
const rutaCon = (ruta, fragmento) =>
  lineas.some((linea) => {
    const normalizada = linea.replace(/\\\//g, '/');
    return normalizada.includes(`/api/v2/${ruta}`) && linea.includes(fragmento);
  });

export default defineSuite({
  id: 'security-authz-matrix',
  area: 'security',
  rules: ['R-GW-1', 'R-GW-3', 'R-DS-5'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-GW-1: el gateway conserva el deny-by-default para /api/v2', () => {
      assert.ok(
        AUTH.includes('patron: /^\\/api\\/v2\\//'),
        'falta la política deny-by-default /^\\/api\\/v2\\//',
      );
      assert.ok(AUTH.includes('roles: []'), 'el deny-by-default debe usar roles: []');
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
          rutaCon(ruta, 'roles: null'),
          `${ruta} no está declarada como pública (roles: null)`,
        );
      }
    });

    test('R-GW-1/R-GW-3: las rutas sensibles exigen roles', () => {
      assert.ok(rutaCon('orders', 'roles: ['), 'orders no tiene una política de roles');
      for (const ruta of ['usuarios', 'finanzas', 'admin', 'storage']) {
        assert.ok(
          AUTH.includes(`/^\\/api\\/v2\\/${ruta}`) && AUTH.includes('roles: ['),
          `${ruta} no tiene una política de roles`,
        );
      }
    });

    test('R-GW-3/R-DS-5: el gateway limpia identidad, tenant y clave interna', () => {
      for (const cabecera of ["'x-user-id'", "'x-user-rol'", "'x-user-email'", 'TENANT_HEADER', "'x-internal-key'"]) {
        assert.ok(PROXY.includes(cabecera), `el scrub no incluye ${cabecera}`);
      }
      assert.ok(PROXY.includes('delete req.headers[h]'), 'el scrub debe borrar las cabeceras');
    });

    test('R-GW-1: el gateway no enruta /internal', () => {
      assert.ok(!PROXY.includes('/^\\/internal'), 'el gateway no debe tener destinos /internal');
    });
  },
});
