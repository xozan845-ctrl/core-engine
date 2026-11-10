import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * RLS del piloto flota (R-SB-2/R-I-12).
 * Dos tenants con filas en todo el esquema field; lectura, escritura y
 * borrado cruzados deben quedar bloqueados por las políticas.
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

const MIGRACIONES = ['02_field.sql', '50_field_rls.sql'];
const TABLAS = [
  'personal', 'clientes', 'vehiculos', 'rutas', 'paradas', 'pedidos',
  'asistencia', 'incidencias', 'tracking', 'visitas', 'cumplimiento',
];

let client;
const tenants = {
  flotaA: { id: randomUUID(), usuarioId: randomUUID() },
  flotaB: { id: randomUUID(), usuarioId: randomUUID() },
};

async function insertarFilas(tenantId) {
  const { rows: [personal] } = await client.query(
    `INSERT INTO field.personal (tenant_id, nombre, cargo) VALUES ($1, 'Conductor', 'conductor') RETURNING id`,
    [tenantId],
  );
  const { rows: [ruta] } = await client.query(
    `INSERT INTO field.rutas (tenant_id, nombre) VALUES ($1, 'Ruta') RETURNING id`,
    [tenantId],
  );
  await client.query(`INSERT INTO field.clientes (tenant_id, nombre_completo) VALUES ($1, 'Cliente')`, [tenantId]);
  await client.query(`INSERT INTO field.vehiculos (tenant_id, placa) VALUES ($1, $2)`, [tenantId, `P-${randomUUID().slice(0, 6)}`]);
  await client.query(`INSERT INTO field.paradas (tenant_id, ruta_id, nombre) VALUES ($1, $2, 'Parada')`, [tenantId, ruta.id]);
  await client.query(`INSERT INTO field.pedidos (tenant_id) VALUES ($1)`, [tenantId]);
  await client.query(
    `INSERT INTO field.asistencia (tenant_id, personal_id, tipo, estado_puntualidad)
     VALUES ($1, $2, 'entrada', 'puntual')`,
    [tenantId, personal.id],
  );
  await client.query(`INSERT INTO field.incidencias (tenant_id, tipo, descripcion) VALUES ($1, 'otro', 'Incidencia')`, [tenantId]);
  await client.query(
    `INSERT INTO field.tracking (tenant_id, personal_id, latitud, longitud) VALUES ($1, $2, 12.1, -86.2)`,
    [tenantId, personal.id],
  );
  await client.query(
    `INSERT INTO field.visitas (tenant_id, personal_id, tipo_actividad) VALUES ($1, $2, 'entrega')`,
    [tenantId, personal.id],
  );
  await client.query(
    `INSERT INTO field.cumplimiento (tenant_id, ruta_id, fecha) VALUES ($1, $2, '2026-11-10')`,
    [tenantId, ruta.id],
  );
  return { personalId: personal.id, rutaId: ruta.id };
}

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    for (const name of MIGRACIONES) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
    tenants.flotaA.filas = await insertarFilas(tenants.flotaA.id);
    tenants.flotaB.filas = await insertarFilas(tenants.flotaB.id);
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});

after(async () => {
  await client?.end();
});

async function conClaims(tenant, fn) {
  await client.query('BEGIN');
  try {
    await client.query('SET LOCAL ROLE core_engine_app');
    await client.query(
      `SELECT set_config('request.jwt.claims', $1, true)`,
      [JSON.stringify({ sub: tenant.usuarioId, rol: 'empresario', tenant_id: tenant.id })],
    );
    return await fn();
  } finally {
    await client.query('ROLLBACK');
  }
}

test('cada tenant solo ve sus filas en todo el esquema field', async () => {
  for (const tabla of TABLAS) {
    const filas = await conClaims(tenants.flotaA, () =>
      client.query(`SELECT tenant_id FROM field.${tabla}`),
    );
    assert.ok(filas.rows.length > 0, `${tabla} debe devolver filas`);
    assert.ok(filas.rows.every((fila) => fila.tenant_id === tenants.flotaA.id), `${tabla} no debe filtrar otro tenant`);
  }
});

test('un tenant no puede insertar ni modificar filas de otro tenant', async () => {
  await assert.rejects(
    conClaims(tenants.flotaA, () =>
      client.query(
        `INSERT INTO field.personal (tenant_id, nombre, cargo) VALUES ($1, 'Intruso', 'conductor')`,
        [tenants.flotaB.id],
      ),
    ),
    (error) => error?.code === '42501',
  );

  const actualizadas = await conClaims(tenants.flotaA, () =>
    client.query(`UPDATE field.personal SET nombre = 'Secuestrado' WHERE id = $1`, [tenants.flotaB.filas.personalId]),
  );
  assert.equal(actualizadas.rowCount, 0);

  const borradas = await conClaims(tenants.flotaA, () =>
    client.query(`DELETE FROM field.rutas WHERE id = $1`, [tenants.flotaB.filas.rutaId]),
  );
  assert.equal(borradas.rowCount, 0);
});

test('el admin global conserva visibilidad de ambos tenants', async () => {
  await client.query('BEGIN');
  try {
    await client.query('SET LOCAL ROLE core_engine_app');
    await client.query(
      `SELECT set_config('request.jwt.claims', $1, true)`,
      [JSON.stringify({ sub: randomUUID(), rol: 'super_admin' })],
    );
    const { rows } = await client.query('SELECT DISTINCT tenant_id FROM field.personal');
    const ids = new Set(rows.map((fila) => fila.tenant_id));
    assert.ok(ids.has(tenants.flotaA.id));
    assert.ok(ids.has(tenants.flotaB.id));
  } finally {
    await client.query('ROLLBACK');
  }
});
