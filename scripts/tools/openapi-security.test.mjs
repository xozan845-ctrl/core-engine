import test from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';

/**
 * Contrato de seguridad de la superficie pública (Tabla 21): solo estas rutas
 * pueden quedar sin `bearer`. Cualquier otra operación protegida por el gateway
 * debe declarar seguridad en OpenAPI.
 */
const RUTAS_PUBLICAS = new Set([
  'GET /api/v1/catalog/productos',
  'GET /api/v1/catalog/productos/{id}',
  'GET /api/v1/tiendas/{id}',
  'POST /api/v1/auth/login',
  'POST /api/v1/auth/refresh',
  'POST /api/v1/auth/registro',
  'POST /api/v1/auth/restablecer-contrasena',
]);

function leerDocumento(nombre) {
  return JSON.parse(readFileSync(resolve(repoRoot, 'docs/api', nombre), 'utf8'));
}

test('debe dejar sin seguridad solo las rutas públicas cuando revisa el contrato público', () => {
  const documento = leerDocumento('core-engine.openapi.json');
  const sinSeguridad = [];

  for (const [ruta, operaciones] of Object.entries(documento.paths)) {
    for (const [metodo, operacion] of Object.entries(operaciones)) {
      if (!operacion.security?.length) {
        sinSeguridad.push(`${metodo.toUpperCase()} ${ruta}`);
      }
    }
  }

  assert.deepEqual(
    sinSeguridad.sort(),
    [...RUTAS_PUBLICAS].sort(),
    'una operación protegida quedó sin declarar seguridad (o una pública declaró bearer)',
  );
});

test('debe exigir x-internal-key en toda operación interna cuando revisa el contrato interno', () => {
  const documento = leerDocumento('core-engine.internal.openapi.json');
  const sinClaveInterna = [];

  for (const [ruta, operaciones] of Object.entries(documento.paths)) {
    for (const [metodo, operacion] of Object.entries(operaciones)) {
      const seguridad = operacion.security ?? documento.security ?? [];
      const exigeClave = seguridad.some((requisito) => 'internalKey' in requisito);
      if (!exigeClave) sinClaveInterna.push(`${metodo.toUpperCase()} ${ruta}`);
    }
  }

  assert.deepEqual(sinClaveInterna, [], 'toda ruta /internal debe exigir x-internal-key');
});

test('debe declarar internalKey en cada operación /internal de los specs por servicio', () => {
  const directorio = resolve(repoRoot, 'docs/openapi');
  const faltantes = [];

  for (const fichero of readdirSync(directorio).filter((f) => f.endsWith('.json'))) {
    const documento = JSON.parse(readFileSync(resolve(directorio, fichero), 'utf8'));
    for (const [ruta, operaciones] of Object.entries(documento.paths ?? {})) {
      if (!ruta.startsWith('/internal')) continue;
      for (const [metodo, operacion] of Object.entries(operaciones)) {
        const seguridad = operacion.security ?? documento.security ?? [];
        if (!seguridad.some((requisito) => 'internalKey' in requisito)) {
          faltantes.push(`${fichero}: ${metodo.toUpperCase()} ${ruta}`);
        }
      }
    }
  }

  assert.deepEqual(faltantes, [], 'los specs por servicio deben declarar x-internal-key');
});
