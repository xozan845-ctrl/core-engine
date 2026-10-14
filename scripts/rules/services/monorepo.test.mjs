import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readJson, readText } from '../../engine/lib/fs.mjs';
import { PACKAGES_DIR, SERVICES } from '../../config/project.mjs';

/**
 * Reglas estructurales del monorepo multipaquete: cada servicio listado en
 * `SERVICES` es un paquete construible y una imagen propia (R-CD-3, R-CD-4).
 */

const serviceDir = (svc) => join(repoRoot, PACKAGES_DIR, svc);

export default defineSuite({
  id: 'services-monorepo',
  area: 'cd',
  rules: ['R-CD-3', 'R-CD-4', 'R-CD-14'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-CD-3: cada servicio tiene package.json, src/main.ts y Dockerfile propio', () => {
      const missing = [];
      for (const svc of SERVICES) {
        for (const rel of ['package.json', join('src', 'main.ts'), 'Dockerfile']) {
          if (!existsSync(join(serviceDir(svc), rel))) {
            missing.push(`${PACKAGES_DIR}/${svc}/${rel}`);
          }
        }
      }
      assert.deepEqual(
        missing,
        [],
        `Faltan artefactos de servicio (R-CD-3):\n${missing.join('\n')}`,
      );
    });

    test('R-CD-3: el nombre del paquete coincide con @core/<servicio>', () => {
      for (const svc of SERVICES) {
        const pkg = readJson(join(serviceDir(svc), 'package.json'));
        assert.equal(
          pkg.name,
          `@core/${svc}`,
          `${PACKAGES_DIR}/${svc}/package.json debe llamarse @core/${svc}`,
        );
      }
    });

    test('R-CD-4: cada Dockerfile compila su servicio, corre como node y arranca su main', () => {
      for (const svc of SERVICES) {
        const df = readText(join(serviceDir(svc), 'Dockerfile'));
        assert.match(
          df,
          new RegExp(`npm run build -w @core/${svc}\\b`),
          `${svc}/Dockerfile no compila @core/${svc}`,
        );
        assert.match(df, /^USER node$/m, `${svc}/Dockerfile no corre como USER node (R-CD-14)`);
        assert.match(
          df,
          new RegExp(`CMD \\["node", "packages/${svc}/dist/main\\.js"\\]`),
          `${svc}/Dockerfile no arranca packages/${svc}/dist/main.js`,
        );
      }
    });

    test('R-CD-4: el puerto EXPOSE es único por servicio', () => {
      const ports = new Map();
      for (const svc of SERVICES) {
        const df = readText(join(serviceDir(svc), 'Dockerfile'));
        const match = df.match(/^EXPOSE\s+(\d+)/m);
        assert.ok(match, `${svc}/Dockerfile sin EXPOSE`);
        const port = match[1];
        assert.ok(
          !ports.has(port),
          `puerto ${port} repetido: ${ports.get(port)} y ${svc} (cada servicio necesita el suyo)`,
        );
        ports.set(port, svc);
      }
    });

    test('R-CD-14: la imagen de runtime no lleva devDependencies', () => {
      // El runtime debe podar el árbol tras compilar: el tooling de test (vitest,
      // swc, eslint…) no debe viajar en la imagen que se publica.
      for (const svc of SERVICES) {
        const df = readText(join(serviceDir(svc), 'Dockerfile'));
        assert.match(
          df,
          /npm prune --omit=dev/,
          `${svc}/Dockerfile debe podar las devDependencies del runtime (npm prune --omit=dev)`,
        );
      }
      for (const rel of ['Dockerfile', join('packages', 'Dockerfile.template')]) {
        const content = readText(join(repoRoot, rel));
        assert.match(
          content,
          /npm prune --omit=dev/,
          `${rel} debe podar las devDependencies del runtime (npm prune --omit=dev)`,
        );
      }
    });
  },
});
