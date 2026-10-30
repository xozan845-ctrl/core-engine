import test from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';

const DIRECTORIO = resolve(repoRoot, 'docs/openapi');

function servicios() {
  return readdirSync(DIRECTORIO, { withFileTypes: true })
    .filter((entrada) => entrada.isDirectory())
    .map((entrada) => entrada.name)
    .sort();
}

function leer(servicio, archivo) {
  return JSON.parse(readFileSync(resolve(DIRECTORIO, servicio, archivo), 'utf8'));
}

function refs(valor, salida = new Set()) {
  if (Array.isArray(valor)) {
    for (const item of valor) refs(item, salida);
    return salida;
  }
  if (!valor || typeof valor !== 'object') return salida;
  for (const [clave, hijo] of Object.entries(valor)) {
    if (clave === '$ref' && typeof hijo === 'string') salida.add(hijo);
    else refs(hijo, salida);
  }
  return salida;
}

test('cada servicio tiene contrato público e interno en su carpeta', () => {
  const encontrados = servicios();
  assert.ok(encontrados.length >= 10, 'deben existir las carpetas de los 10 servicios');
  for (const servicio of encontrados) {
    for (const archivo of ['public.openapi.json', 'internal.openapi.json']) {
      const documento = leer(servicio, archivo);
      assert.equal(documento['x-servicio'], servicio, `${servicio}/${archivo}: x-servicio`);
      const superficie = archivo.startsWith('public') ? 'publica' : 'interna';
      assert.equal(documento['x-superficie'], superficie, `${servicio}/${archivo}: x-superficie`);
    }
  }
});

test('no quedan specs planos ni rutas fuera de la superficie declarada', () => {
  const planos = readdirSync(DIRECTORIO).filter((nombre) => nombre.endsWith('.json'));
  assert.deepEqual(planos, [], 'docs/openapi debe contener solo carpetas de servicio');

  for (const servicio of servicios()) {
    const publico = leer(servicio, 'public.openapi.json');
    for (const ruta of Object.keys(publico.paths ?? {})) {
      assert.match(ruta, /^\/api\/v1(?:\/|$)/, `${servicio}: ruta pública inválida ${ruta}`);
    }
    const interno = leer(servicio, 'internal.openapi.json');
    for (const ruta of Object.keys(interno.paths ?? {})) {
      assert.match(ruta, /^\/internal(?:\/|$)/, `${servicio}: ruta interna inválida ${ruta}`);
    }
  }
});

test('todos los $ref resuelven dentro del mismo archivo', () => {
  for (const servicio of servicios()) {
    for (const archivo of ['public.openapi.json', 'internal.openapi.json']) {
      const documento = leer(servicio, archivo);
      const schemas = documento.components?.schemas ?? {};
      const faltantes = [];
      for (const ref of refs(documento)) {
        const prefijo = '#/components/schemas/';
        if (!ref.startsWith(prefijo)) {
          faltantes.push(`${ref} (tipo de $ref no soportado)`);
          continue;
        }
        const nombre = decodeURIComponent(ref.slice(prefijo.length));
        if (!(nombre in schemas)) faltantes.push(ref);
      }
      assert.deepEqual(
        faltantes,
        [],
        `${servicio}/${archivo}: referencias sin schema:\n${faltantes.join('\n')}`,
      );
    }
  }
});
