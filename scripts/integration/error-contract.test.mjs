import assert from 'node:assert/strict';
import test, { after, before } from 'node:test';
import request from 'supertest';

let app;
let http;

class ErrorContractController {
  forbidden() {
    throw new (this.shared.ForbiddenError)('Rol insuficiente.');
  }

  notFound() {
    throw new (this.shared.NotFoundError)('Recurso', 'ausente');
  }

  conflict() {
    throw new (this.shared.ConflictError)('La operación entra en conflicto.');
  }

  internal() {
    throw new Error('detalle interno que nunca debe exponerse');
  }
}

before(async () => {
  const [{ Controller, Get, Module }, { NestFactory }, shared] = await Promise.all([
    import('@nestjs/common'),
    import('@nestjs/core'),
    import('@core/shared'),
  ]);

  ErrorContractController.prototype.shared = shared;
  Controller('contract-errors')(ErrorContractController);
  for (const [name, path] of [
    ['forbidden', '403'],
    ['notFound', '404'],
    ['conflict', '409'],
    ['internal', '500'],
  ]) {
    const descriptor = Object.getOwnPropertyDescriptor(ErrorContractController.prototype, name);
    Get(path)(ErrorContractController.prototype, name, descriptor);
  }

  class IntegrationModule {}
  Module({ controllers: [ErrorContractController] })(IntegrationModule);
  app = await NestFactory.create(IntegrationModule, { logger: false });
  app.useGlobalFilters(new shared.DomainErrorFilter());
  await app.init();
  http = request(app.getHttpServer());
});

after(async () => {
  await app?.close();
});

test('Contrato G-5/R-C-8: 403, 404, 409 y 500 tienen shape normalizado', async () => {
  const casos = [
    ['/contract-errors/403', 403, 'ACCESO_DENEGADO'],
    ['/contract-errors/404', 404, 'NO_ENCONTRADO'],
    ['/contract-errors/409', 409, 'CONFLICTO'],
    ['/contract-errors/500', 500, 'ERROR_INTERNO'],
  ];

  for (const [ruta, status, codigo] of casos) {
    const respuesta = await http.get(ruta).expect(status);
    assert.equal(respuesta.body.codigo, codigo);
    assert.equal(typeof respuesta.body.mensaje, 'string');
    assert.ok(!('stack' in respuesta.body));
    assert.ok(!JSON.stringify(respuesta.body).includes('detalle interno'));
  }
});
