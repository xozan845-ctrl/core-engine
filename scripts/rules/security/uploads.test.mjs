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
  join(repoRoot, 'packages/storage-service/src/config.rs'),
);
const SERVICIO = readText(
  join(repoRoot, 'packages/storage-service/src/storage/objetos.rs'),
);

export default defineSuite({
  id: 'security-uploads',
  area: 'security',
  rules: ['R-DS-4'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-DS-4: la whitelist de content-types y los límites son configurables', () => {
      assert.ok(CONFIG.includes('content_types_permitidos'), 'falta content_types_permitidos');
      assert.ok(CONFIG.includes('STORAGE_CONTENT_TYPES_PERMITIDOS'), 'la whitelist no es configurable');
      assert.ok(CONFIG.includes('max_objeto_bytes'), 'falta max_objeto_bytes');
      assert.ok(CONFIG.includes('max_video_bytes'), 'falta max_video_bytes');
    });

    test('R-DS-4: se valida el tipo al solicitar y al confirmar', () => {
      const comprobaciones = SERVICIO.split('content_types_permitidos.contains').length - 1;
      assert.ok(comprobaciones >= 2, 'el content-type debe validarse en solicitud y confirmación');
      assert.ok(
        SERVICIO.includes('.estadisticas('),
        'la confirmación debe consultar el objeto real en storage',
      );
      assert.ok(SERVICIO.includes('maximo_para'), 'faltan los límites por tipo');
    });

    test('R-DS-4/ADR-28: SVG bloqueado y nombres saneados', () => {
      assert.ok(
        SERVICIO.includes('image/svg+xml'),
        'SVG debe rechazarse en la superficie pública',
      );
      assert.ok(SERVICIO.includes('sanear_nombre'), 'falta el saneado del nombre');
      assert.ok(SERVICIO.includes('sanear_carpeta'), 'falta el saneado de la carpeta');
      assert.ok(
        SERVICIO.includes("estado = 'activo' AND publico = true"),
        'la lectura pública debe exigir estado activo y publico = true',
      );
    });
  },
});
