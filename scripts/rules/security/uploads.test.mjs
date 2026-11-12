import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

/**
 * Subida de archivos (R-DS-4 / ADR-28).
 *
 * El servicio debe mantener la whitelist de content-types, revalidar el objeto
 * real al confirmar, bloquear SVG en la superficie pública y sanear nombres.
 */

const CONFIG = readText(
  join(repoRoot, 'packages/storage-service/src/config/storage.config.ts'),
);
const SERVICIO = readText(
  join(repoRoot, 'packages/storage-service/src/objetos/objetos.service.ts'),
);

export default defineSuite({
  id: 'security-uploads',
  area: 'security',
  rules: ['R-DS-4'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-4: la whitelist de content-types y los límites son configurables', () => {
      assert.ok(CONFIG.includes('contentTypesPermitidos'), 'falta contentTypesPermitidos');
      assert.ok(CONFIG.includes('STORAGE_CONTENT_TYPES_PERMITIDOS'), 'la whitelist no es configurable');
      assert.ok(CONFIG.includes('maxObjetoBytes'), 'falta maxObjetoBytes');
      assert.ok(CONFIG.includes('maxVideoBytes'), 'falta maxVideoBytes');
    });

    test('R-DS-4: se valida el tipo al solicitar y al confirmar', () => {
      const comprobaciones = SERVICIO.split('contentTypesPermitidos.includes').length - 1;
      assert.ok(comprobaciones >= 2, 'el content-type debe validarse en solicitud y confirmación');
      assert.ok(
        SERVICIO.includes('this.almacen.estadisticas'),
        'la confirmación debe consultar el objeto real en storage',
      );
      assert.ok(SERVICIO.includes('maximoPara'), 'faltan los límites por tipo');
    });

    test('R-DS-4/ADR-28: SVG bloqueado y nombres saneados', () => {
      assert.ok(
        SERVICIO.includes("tipo === 'image/svg+xml'"),
        'SVG debe rechazarse en la superficie pública',
      );
      assert.ok(SERVICIO.includes('sanearNombre'), 'falta el saneado del nombre');
      assert.ok(SERVICIO.includes('sanearCarpeta'), 'falta el saneado de la carpeta');
      assert.ok(
        SERVICIO.includes("estado = 'activo' AND publico = true"),
        'la lectura pública debe exigir estado activo y publico = true',
      );
    });
  },
});
