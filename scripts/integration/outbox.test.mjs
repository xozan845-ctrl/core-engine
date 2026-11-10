import assert from 'node:assert/strict';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * R-RB-3: dos pollers del Outbox compiten sobre la misma tabla.
 *
 * El primero reclama los eventos con `FOR UPDATE SKIP LOCKED`; el segundo no
 * debe ver esas filas ni publicarlas. La prueba usa PostgreSQL real y un
 * broker simulado que bloquea la primera publicación para garantizar que el
 * segundo poller entre mientras el primero todavía mantiene el lock.
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

const ESQUEMA = 'outbox_it';
const TABLA = `${ESQUEMA}.eventos`;
const TOTAL = 20;

let client;
let pgService;
let OutboxService;

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query(`DROP SCHEMA IF EXISTS ${ESQUEMA} CASCADE`);
  await client.query(`CREATE SCHEMA ${ESQUEMA}`);
  await client.query(`
    CREATE TABLE ${TABLA} (
      event_id     uuid PRIMARY KEY,
      tipo         text NOT NULL,
      payload      jsonb NOT NULL,
      estado       text NOT NULL DEFAULT 'pendiente'
                     CHECK (estado IN ('pendiente', 'publicado')),
      creado_en    timestamptz NOT NULL DEFAULT NOW(),
      publicado_en timestamptz
    )
  `);

  const shared = await import('@core/shared');
  OutboxService = shared.OutboxService;
  pgService = new shared.PgService({ url: connectionString });
  await pgService.onModuleInit();
});

after(async () => {
  await pgService?.onModuleDestroy();
  try {
    await client?.query(`DROP SCHEMA IF EXISTS ${ESQUEMA} CASCADE`);
  } finally {
    await client?.end();
  }
});

test('dos pollers concurrentes publican cada evento exactamente una vez', async () => {
  const emisor = new OutboxService(pgService, null, TABLA);
  const ids = [];
  for (let i = 0; i < TOTAL; i += 1) {
    ids.push(await emisor.insertar('order.created', { order_id: `orden-${i}` }));
  }

  const publicados = new Map();
  let primeraIniciada;
  let liberarPrimera;
  const esperaPrimera = new Promise((resolve) => {
    primeraIniciada = resolve;
  });
  const esperaLiberacion = new Promise((resolve) => {
    liberarPrimera = resolve;
  });
  let llamadasA = 0;
  const rabbitA = {
    publicar: async (evento) => {
      // La primera publicación de A se queda en vuelo hasta que B ya intentó
      // reclamar; así el lock de A está garantizado durante la carrera.
      if (llamadasA === 0) {
        primeraIniciada();
        await esperaLiberacion;
      }
      llamadasA += 1;
      publicados.set(evento.event_id, (publicados.get(evento.event_id) ?? 0) + 1);
      return true;
    },
  };
  const rabbitB = {
    publicar: async (evento) => {
      publicados.set(evento.event_id, (publicados.get(evento.event_id) ?? 0) + 1);
      return true;
    },
  };
  const outboxA = new OutboxService(pgService, rabbitA, TABLA);
  const outboxB = new OutboxService(pgService, rabbitB, TABLA);

  const pollerA = publicarPendientes(outboxA);
  await esperaPrimera;
  const pollerB = publicarPendientes(outboxB);
  await pollerB;
  liberarPrimera();
  await pollerA;

  assert.equal(publicados.size, TOTAL, 'deben publicarse todos los eventos');
  for (const id of ids) {
    assert.equal(publicados.get(id), 1, `el evento ${id} se publicó más de una vez`);
  }

  const { rows } = await client.query(
    `SELECT estado, COUNT(*)::int AS total, COUNT(publicado_en)::int AS confirmados
       FROM ${TABLA}
      GROUP BY estado
      ORDER BY estado`,
  );
  assert.deepEqual(rows, [{ estado: 'publicado', total: TOTAL, confirmados: TOTAL }]);
});

function publicarPendientes(servicio) {
  return servicio.publicarPendientes();
}
