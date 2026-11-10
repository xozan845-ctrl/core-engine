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

/**
 * Migraciones que necesita la configuración de sitio: esquemas base, fondo de
 * plataforma, personalización heredada (22 depende de 19) y panel de IA.
 */
const MIGRACIONES = [
  '01_esquemas.sql',
  '16_media_background_plataforma.sql',
  '19_personalizacion_tienda.sql',
  '22_herencia_tema.sql',
  '23_configuracion_panel_ai.sql',
];

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
    const migraciones = existe
      ? MIGRACIONES.filter((name) => name !== '01_esquemas.sql')
      : MIGRACIONES;
    for (const name of migraciones) {
      const sql = await readFile(resolve(repoRoot, `infra/db/init/${name}`), 'utf8');
      await client.query(sql);
    }
    await client.query(
      `UPDATE identity.configuracion_plataforma
       SET background_objeto_id = NULL, background_media_type = NULL,
           background_poster_objeto_id = NULL, background_alt = '',
           background_overlay = 35, background_position = 'center',
           background_activo = false,
           panel_ai_background_objeto_id = NULL,
           panel_ai_background_media_type = NULL,
           panel_ai_background_poster_objeto_id = NULL,
           panel_ai_background_alt = '',
           panel_ai_background_overlay = 35,
           panel_ai_background_position = 'center',
           panel_ai_background_activo = false,
           actualizado_por = NULL`,
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
  const respuesta = await http.get('/api/v2/configuracion/sitio').expect(200);
  assert.deepEqual(
    {
      background_objeto_id: respuesta.body.background_objeto_id,
      background_media_type: respuesta.body.background_media_type,
      background_poster_objeto_id: respuesta.body.background_poster_objeto_id,
      background_alt: respuesta.body.background_alt,
      background_overlay: respuesta.body.background_overlay,
      background_position: respuesta.body.background_position,
      background_activo: respuesta.body.background_activo,
      background_url: respuesta.body.background_url,
      background_poster_url: respuesta.body.background_poster_url,
      panel_ai_background_objeto_id: respuesta.body.panel_ai_background_objeto_id,
      panel_ai_background_media_type: respuesta.body.panel_ai_background_media_type,
      panel_ai_background_poster_objeto_id: respuesta.body.panel_ai_background_poster_objeto_id,
      panel_ai_background_alt: respuesta.body.panel_ai_background_alt,
      panel_ai_background_overlay: respuesta.body.panel_ai_background_overlay,
      panel_ai_background_position: respuesta.body.panel_ai_background_position,
      panel_ai_background_activo: respuesta.body.panel_ai_background_activo,
      panel_ai_background_url: respuesta.body.panel_ai_background_url,
      panel_ai_background_poster_url: respuesta.body.panel_ai_background_poster_url,
    },
    {
      background_objeto_id: null,
      background_media_type: null,
      background_poster_objeto_id: null,
      background_alt: '',
      background_overlay: 35,
      background_position: 'center',
      background_activo: false,
      background_url: null,
      background_poster_url: null,
      panel_ai_background_objeto_id: null,
      panel_ai_background_media_type: null,
      panel_ai_background_poster_objeto_id: null,
      panel_ai_background_alt: '',
      panel_ai_background_overlay: 35,
      panel_ai_background_position: 'center',
      panel_ai_background_activo: false,
      panel_ai_background_url: null,
      panel_ai_background_poster_url: null,
    },
  );
});

test('Contrato G-5: PATCH no autenticado devuelve error normalizado', async () => {
  const respuesta = await http
    .patch('/api/v2/configuracion/sitio')
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
    .patch('/api/v2/configuracion/sitio')
    .set(headers)
    .send({ background_alt: '  plataforma  ', background_overlay: 60, background_activo: false })
    .expect(200);
  assert.equal(actualizado.body.background_alt, 'plataforma');
  assert.equal(actualizado.body.background_overlay, 60);

  const lectura = await http.get('/api/v2/configuracion/sitio').expect(200);
  assert.equal(lectura.body.background_alt, 'plataforma');
  assert.equal(lectura.body.background_overlay, 60);
});

test('Contrato G-5: validación de entrada mantiene el shape de error', async () => {
  const respuesta = await http
    .patch('/api/v2/configuracion/sitio')
    .set('x-user-id', '00000000-0000-4000-8000-000000000001')
    .set('x-user-rol', 'admin')
    .send({ background_overlay: 101 })
    .expect(400);
  assert.equal(respuesta.body.codigo, 'VALIDACION');
  assert.equal(typeof respuesta.body.mensaje, 'string');
  assert.ok(Array.isArray(respuesta.body.detalles));
  assert.ok(!('stack' in respuesta.body));
});
