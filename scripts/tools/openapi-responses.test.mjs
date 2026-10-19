import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';

/**
 * Ratchet de respuestas OpenAPI: los servicios migrados deben documentar el
 * schema de éxito (200/201) de todas sus operaciones. Al migrar un servicio se
 * añade aquí; nunca se quita uno ya migrado.
 */
const SERVICIOS_MIGRADOS = [
  'catalog-service',
  'identity-service',
  'logistics-service',
  'market-intelligence-service',
  'orders-service',
  'commissions-service',
  'stores-service',
  'storage-service',
];

test('debe documentar schema de éxito en los servicios migrados cuando se exporta OpenAPI', () => {
  for (const servicio of SERVICIOS_MIGRADOS) {
    const documento = JSON.parse(
      readFileSync(resolve(repoRoot, 'docs/openapi', `${servicio}.json`), 'utf8'),
    );
    const faltantes = [];

    for (const [ruta, operaciones] of Object.entries(documento.paths ?? {})) {
      for (const [metodo, operacion] of Object.entries(operaciones)) {
        const exito = operacion.responses?.['200'] ?? operacion.responses?.['201'];
        const sinContenido = operacion.responses?.['204'] !== undefined;
        if (!exito?.content && !sinContenido) {
          faltantes.push(`${metodo.toUpperCase()} ${ruta}`);
        }
      }
    }

    assert.deepEqual(
      faltantes,
      [],
      `${servicio}: operaciones sin schema de éxito (R-DO-6):\n${faltantes.join('\n')}`,
    );
  }
});
