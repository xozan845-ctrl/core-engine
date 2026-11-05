import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import test, { after, before } from 'node:test';
import pg from 'pg';
import request from 'supertest';

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (
  databaseUrl.pathname !== '/core_engine_test' ||
  !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)
) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

process.env.DATABASE_URL = connectionString;
process.env.RABBITMQ_URL = '';
process.env.STORAGE_SERVICE_URL = 'http://127.0.0.1:9';

const repoRoot = resolve(new URL('../..', import.meta.url).pathname);
let client;
let app;
let http;

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    const { rows: [{ existe }] } = await client.query(
      `SELECT to_regnamespace('identity') IS NOT NULL AS existe`,
    );
    if (!existe) {
      const base = await readFile(resolve(repoRoot, 'infra/db/init/01_esquemas.sql'), 'utf8');
      await client.query(base);
    }
    const sql = await readFile(
      resolve(repoRoot, 'infra/db/init/16_media_background_plataforma.sql'),
      'utf8',
    );
    await client.query(sql);
    await client.query(
      `UPDATE identity.configuracion_plataforma
       SET background_objeto_id = NULL, background_media_type = NULL,
           background_poster_objeto_id = NULL, background_alt = '',
           background_overlay = 35, background_position = 'center',
           background_activo = false, actualizado_por = NULL`,
    );
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const [{ Module }, { NestFactory }, shared, { ConfiguracionSitioModule }] = await Promise.all([
    import('@nestjs/common'),
    import('@nestjs/core'),
    import('@core/shared'),
    import('../../packages/identity-service/dist/configuracion-sitio/configuracion-sitio.module.js'),
  ]);

  class IntegrationModule {}
  Module({ imports: [shared.PgModule, shared.MetricsModule, ConfiguracionSitioModule] })(
    IntegrationModule,
  );

  app = await NestFactory.create(IntegrationModule, { logger: false });
  app.useGlobalPipes(new shared.DtoValidationPipe());
  app.useGlobalFilters(new shared.DomainErrorFilter());
  app.useGlobalGuards(new shared.RolesGuard());
  app.useGlobalInterceptors(
    new shared.TrazabilidadInterceptor(app.get(shared.MetricsService), 'identity-service'),
  );
  await app.init();
  http = request(app.getHttpServer());
});

after(async () => {
  await app?.close();
  await client?.end();
});

test('API+BD: GET de configuración devuelve valores persistidos', async () => {
  const respuesta = await http.get('/api/v1/configuracion/sitio').expect(200);
  assert.deepEqual(respuesta.body, {
    background_objeto_id: null,
    background_media_type: null,
    background_poster_objeto_id: null,
    background_alt: '',
    background_overlay: 35,
    background_position: 'center',
    background_activo: false,
    background_url: null,
    background_poster_url: null,
    actualizado_en: respuesta.body.actualizado_en,
  });
});

test('Contrato G-5: PATCH no autenticado devuelve error normalizado', async () => {
  const respuesta = await http
    .patch('/api/v1/configuracion/sitio')
    .send({ background_alt: 'sin acceso' })
    .expect(401);
  assert.equal(respuesta.body.codigo, 'NO_AUTORIZADO');
  assert.equal(typeof respuesta.body.mensaje, 'string');
  assert.ok(!('stack' in respuesta.body));
});

test('API+BD: PATCH admin persiste y GET devuelve el cambio', async () => {
  const headers = {
    'x-user-id': '00000000-0000-4000-8000-000000000001',
    'x-user-rol': 'admin',
  };
  const actualizado = await http
    .patch('/api/v1/configuracion/sitio')
    .set(headers)
    .send({ background_alt: '  plataforma  ', background_overlay: 60, background_activo: false })
    .expect(200);
  assert.equal(actualizado.body.background_alt, 'plataforma');
  assert.equal(actualizado.body.background_overlay, 60);

  const lectura = await http.get('/api/v1/configuracion/sitio').expect(200);
  assert.equal(lectura.body.background_alt, 'plataforma');
  assert.equal(lectura.body.background_overlay, 60);
});

test('Contrato G-5: validación de entrada mantiene el shape de error', async () => {
  const respuesta = await http
    .patch('/api/v1/configuracion/sitio')
    .set('x-user-id', '00000000-0000-4000-8000-000000000001')
    .set('x-user-rol', 'admin')
    .send({ background_overlay: 101 })
    .expect(400);
  assert.equal(respuesta.body.codigo, 'VALIDACION');
  assert.equal(typeof respuesta.body.mensaje, 'string');
  assert.ok(Array.isArray(respuesta.body.detalles));
  assert.ok(!('stack' in respuesta.body));
});
