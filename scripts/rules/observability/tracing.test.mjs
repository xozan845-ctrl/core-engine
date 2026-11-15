import { join, relative } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { readText, walkFiles } from '../../engine/lib/fs.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { PACKAGES_DIR, SERVICES } from '../../config/project.mjs';

const packagesRoot = join(repoRoot, PACKAGES_DIR);

export default defineSuite({
  id: 'observability-tracing',
  area: 'observability',
  rules: ['R-OB-1', 'R-OB-2'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-OB-1: los servicios no emiten console logs sin JSON estructurado', () => {
      const problems = [];
      for (const service of SERVICES) {
        const files = walkFiles(
          join(packagesRoot, service, 'src'),
          (name) => name.endsWith('.ts') && !name.endsWith('.spec.ts'),
        ).filter((file) => !file.endsWith('swagger-export.ts'));

        for (const file of files) {
          const lines = readText(file).split('\n');
          lines.forEach((line, index) => {
            if (!/console\.(log|warn|error|debug)\s*\(/.test(line)) return;
            if (/console\.error\(JSON\.stringify\(/.test(line)) return;
            problems.push(`${relative(repoRoot, file)}:${index + 1}`);
          });
        }
      }
      assert.deepEqual(
        problems,
        [],
        `console sin JSON estructurado en código de runtime:\n${problems.join('\n')}`,
      );
    });

    test('R-OB-2: el gateway crea, devuelve y propaga x-request-id por HTTP', () => {
      const http = readText(join(packagesRoot, 'api-gateway', 'src', 'http', 'mod.rs'));
      const proxy = readText(join(packagesRoot, 'api-gateway', 'src', 'proxy.rs'));
      assert.match(http, /Uuid::new_v4\(\)/, 'el gateway no genera el ID');
      assert.match(
        http,
        /request\.headers_mut\(\)\.insert\("x-request-id"/,
        'el gateway no inyecta el ID en la petición',
      );
      assert.match(
        http,
        /response\.headers_mut\(\)\.insert\("x-request-id"/,
        'el gateway no devuelve el ID al cliente',
      );
      assert.match(
        proxy,
        /headers\.insert\("x-request-id"/,
        'el proxy no reenvía el ID al servicio destino',
      );
    });

    test('R-OB-2: los servicios conservan x-request-id en su contexto', () => {
      const http = readText(join(packagesRoot, 'identity-service', 'src', 'http', 'mod.rs'));
      assert.match(http, /request_id/, 'falta el request_id en el contexto');
      assert.match(http, /x-request-id/, 'falta la cabecera x-request-id');
      assert.match(http, /ejecutar_con_contexto\(contexto,/, 'falta el contexto de correlación');
    });

    test('R-OB-2: outbox y RabbitMQ propagan request_id en payload y header', () => {
      const outbox = readText(join(packagesRoot, 'orders-service', 'src', 'outbox.rs'));
      const rabbit = readText(join(packagesRoot, 'orders-service', 'src', 'rabbit.rs'));
      assert.match(outbox, /request_id/, 'el outbox no persiste el request_id');
      assert.match(outbox, /json!\(ctx\.request_id\)/, 'el outbox no usa el request_id');
      assert.match(rabbit, /x-request-id/, 'RabbitMQ no publica x-request-id');
    });
  },
});
