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
      const interceptor = readText(
        join(packagesRoot, 'shared', 'src', 'http', 'validation.pipe.ts'),
      );
      assert.match(interceptor, /req\.headers\?\.\[['"]x-request-id['"]\]/);
      assert.match(interceptor, /res\.setHeader\(['"]x-request-id['"],\s*request_id\)/);
      assert.match(interceptor, /ejecutarConContexto\(contexto,/);
    });

    test('R-OB-2: outbox y RabbitMQ propagan request_id en payload y header', () => {
      const outbox = readText(join(packagesRoot, 'shared', 'src', 'outbox', 'outbox.service.ts'));
      const events = readText(join(packagesRoot, 'shared', 'src', 'events', 'contracts.ts'));
      const rabbit = readText(
        join(packagesRoot, 'shared', 'src', 'rabbitmq', 'rabbit.service.ts'),
      );
      assert.match(events, /request_id\?:\s*string/);
      assert.match(outbox, /ctx\?\.request_id\s*\?\s*\{\s*request_id:\s*ctx\.request_id\s*\}/);
      assert.match(outbox, /JSON\.stringify\(evento\)/, 'outbox no persiste el evento correlacionado');
      assert.match(
        rabbit,
        /\{\s*['"]x-request-id['"]:\s*evento\.request_id\s*\}/,
        'RabbitMQ no publica x-request-id como header',
      );
    });
  },
});
