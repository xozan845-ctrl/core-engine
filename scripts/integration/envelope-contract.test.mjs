import assert from 'node:assert/strict';
import test, { after, before } from 'node:test';
import request from 'supertest';

/**
 * R-C-1: el envelope se aplica en un Nest real, no solo en el helper puro.
 * No usa BD ni broker: crea un controller mínimo y el interceptor global.
 */

let app;
let http;

class EnvelopeDemoController {
  demo() {
    return { ok: true, valor: 42 };
  }

  interno() {
    return { ok: true, interno: true };
  }

  vacio() {
    return undefined;
  }
}

before(async () => {
  const [{ Controller, Get, HttpCode, Module }, { NestFactory }, shared] = await Promise.all([
    import('@nestjs/common'),
    import('@nestjs/core'),
    import('@core/shared'),
  ]);

  Controller()(EnvelopeDemoController);
  Get('api/v2/envelope-demo')(EnvelopeDemoController.prototype, 'demo', Object.getOwnPropertyDescriptor(EnvelopeDemoController.prototype, 'demo'));
  Get('internal/envelope-demo')(EnvelopeDemoController.prototype, 'interno', Object.getOwnPropertyDescriptor(EnvelopeDemoController.prototype, 'interno'));
  Get('api/v2/envelope-vacio')(EnvelopeDemoController.prototype, 'vacio', Object.getOwnPropertyDescriptor(EnvelopeDemoController.prototype, 'vacio'));
  HttpCode(204)(EnvelopeDemoController.prototype, 'vacio', Object.getOwnPropertyDescriptor(EnvelopeDemoController.prototype, 'vacio'));

  class IntegrationModule {}
  Module({ controllers: [EnvelopeDemoController] })(IntegrationModule);
  app = await NestFactory.create(IntegrationModule, { logger: false });
  app.useGlobalFilters(new shared.DomainErrorFilter());
  app.useGlobalInterceptors(new shared.EnvelopeInterceptor());
  await app.init();
  http = request(app.getHttpServer());
});

after(async () => {
  await app?.close();
});

test('R-C-1: una respuesta pública sale con { data, meta }', async () => {
  const respuesta = await http.get('/api/v2/envelope-demo').expect(200);

  assert.deepEqual(respuesta.body.data, { ok: true, valor: 42 });
  assert.equal(respuesta.body.meta.statusCode, 200);
  assert.equal(respuesta.body.meta.method, 'GET');
  assert.equal(respuesta.body.meta.path, '/api/v2/envelope-demo');
  assert.match(respuesta.body.meta.timestamp, /^\d{4}-\d{2}-\d{2}T.*Z$/);
});

test('R-C-1: /internal no se envuelve', async () => {
  const respuesta = await http.get('/internal/envelope-demo').expect(200);
  assert.deepEqual(respuesta.body, { ok: true, interno: true });
});

test('R-C-1: 204 no se envuelve ni produce cuerpo', async () => {
  const respuesta = await http.get('/api/v2/envelope-vacio').expect(204);
  assert.equal(respuesta.text, '');
});
