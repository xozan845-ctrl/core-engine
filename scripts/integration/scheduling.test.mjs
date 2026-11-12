import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

let client;

before(async () => {
  client = new pg.Client({ connectionString });
  try {
    await client.connect();
    await client.query('SELECT pg_advisory_lock(38194201)');
    const { rows: [{ existe }] } = await client.query(
      'SELECT to_regnamespace(\'identity\') IS NOT NULL AS existe',
    );
    const migraciones = existe ? ['28_scheduling.sql'] : ['01_esquemas.sql', '24_multi_vertical.sql', '28_scheduling.sql'];
    for (const name of migraciones) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
  } catch (error) {
    console.error('scheduling integration setup failed:', error);
    throw error;
  } finally {
    await client?.query('SELECT pg_advisory_unlock(38194201)').catch(() => undefined);
  }
});

after(async () => {
  await client?.end();
});

test('la exclusion GiST impide dos citas concurrentes para el mismo recurso', { concurrency: false }, async () => {
  const empresaId = randomUUID();
  const recursoId = randomUUID();
  const inicia = '2032-04-12T14:00:00Z';
  const termina = '2032-04-12T15:00:00Z';
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    await client.query('BEGIN');
    await client.query(
      `INSERT INTO scheduling.recursos (id, empresa_id, tipo, nombre)
       VALUES ($1, $2, 'sala', $3)`,
      [recursoId, empresaId, `Sala ${recursoId}`],
    );
    await client.query(
      `INSERT INTO scheduling.disponibilidad
        (empresa_id, recurso_id, dia_semana, inicia_hora, termina_hora)
       VALUES ($1, $2, 1, '08:00', '18:00')`,
      [empresaId, recursoId],
    );
    await client.query(
      `INSERT INTO scheduling.citas
        (empresa_id, recurso_id, inicia_en, termina_en)
       VALUES ($1, $2, $3, $4)`,
      [empresaId, recursoId, inicia, termina],
    );
    await assert.rejects(
      client.query(
        `INSERT INTO scheduling.citas
          (empresa_id, recurso_id, inicia_en, termina_en)
         VALUES ($1, $2, $3, $4)`,
        [empresaId, recursoId, inicia, termina],
      ),
      (error) => error?.code === '23P01',
    );
    await client.query('ROLLBACK');
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
