import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { PACKAGES_DIR, SERVICES } from '../project.config.mjs';
import { repositoryRoot } from './helpers/repo.mjs';

/**
 * Reglas estructurales del monorepo multipaquete: cada servicio listado en
 * `SERVICES` es un paquete construible y una imagen propia (R-CD-3, R-CD-4).
 */

const serviceDir = (svc) => join(repositoryRoot, PACKAGES_DIR, svc);

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
    const pkg = JSON.parse(
      readFileSync(join(serviceDir(svc), 'package.json'), 'utf8'),
    );
    assert.equal(
      pkg.name,
      `@core/${svc}`,
      `${PACKAGES_DIR}/${svc}/package.json debe llamarse @core/${svc}`,
    );
  }
});

test('R-CD-4: cada Dockerfile compila su servicio, corre como node y arranca su main', () => {
  for (const svc of SERVICES) {
    const df = readFileSync(join(serviceDir(svc), 'Dockerfile'), 'utf8');
    assert.match(
      df,
      new RegExp(`npm run build -w @core/${svc}\\b`),
      `${svc}/Dockerfile no compila @core/${svc}`,
    );
    assert.match(df, /^USER node$/m, `${svc}/Dockerfile no corre como USER node (R-CD-22)`);
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
    const df = readFileSync(join(serviceDir(svc), 'Dockerfile'), 'utf8');
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
