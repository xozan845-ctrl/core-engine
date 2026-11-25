#!/usr/bin/env node
/**
 * Prueba negativa A/B de RLS sobre una base con `99_rls.sql` y
 * `99z_rls_hardening.sql` aplicados.
 *
 * Uso:
 *   RLS_TEST_DATABASE_URL=postgres://owner:...@host/db \
 *   RLS_TEST_APP_URL=postgres://core_engine_app:...@host/db \
 *   node scripts/tools/rls-ab-test.mjs
 *
 * Comprueba que un usuario del tenant A no puede leer, actualizar ni borrar
 * filas del tenant B en usuarios, pedidos, finanzas y storage. También
 * comprueba que un super_admin conserva visibilidad global.
 */
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import pg from 'pg';

const ownerUrl = process.env.RLS_TEST_DATABASE_URL;
const appUrl = process.env.RLS_TEST_APP_URL;
if (!ownerUrl || !appUrl) {
  console.error('Faltan RLS_TEST_DATABASE_URL y RLS_TEST_APP_URL.');
  process.exit(1);
}

const owner = new pg.Client({ connectionString: ownerUrl });
const app = new pg.Client({ connectionString: appUrl });
const ids = {
  tenantA: randomUUID(),
  tenantB: randomUUID(),
  userA: randomUUID(),
  userB: randomUUID(),
  pagoA: randomUUID(),
  pagoB: randomUUID(),
  comprobanteA: randomUUID(),
  comprobanteB: randomUUID(),
  objetoA: randomUUID(),
  objetoB: randomUUID(),
};

async function conClaims(claims, fn) {
  await app.query('BEGIN');
  try {
    await app.query(`SELECT set_config('request.jwt.claims', $1, true)`, [
      JSON.stringify(claims),
    ]);
    return await fn();
  } finally {
    await app.query('ROLLBACK');
  }
}

async function filasDe(tabla, columna, tenantId) {
  const { rows } = await conClaims(
    { sub: tenantId === ids.tenantA ? ids.userA : ids.userB, rol: 'empresario', tenant_id: tenantId },
    () => app.query(`SELECT ${columna} FROM ${tabla}`),
  );
  return rows;
}

async function main() {
  await owner.connect();
  await app.connect();

  // Semilla como owner.
  await owner.query('BEGIN');
  try {
    await owner.query(
      `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol, tenant_id)
       VALUES ($1, 'A', $2, 'x', 'empresario', $3),
              ($4, 'B', $5, 'x', 'empresario', $6)`,
      [
        ids.userA,
        `rls-a-${ids.userA}@example.test`,
        ids.tenantA,
        ids.userB,
        `rls-b-${ids.userB}@example.test`,
        ids.tenantB,
      ],
    );

    await owner.query(
      `INSERT INTO orders.pagos (id, order_id, empresa_id, metodo_codigo, estado, monto_cents)
       VALUES ($1, $2, $3, 'efectivo', 'confirmado', 100),
              ($4, $5, $6, 'efectivo', 'confirmado', 200)`,
      [
        ids.pagoA,
        randomUUID(),
        ids.tenantA,
        ids.pagoB,
        randomUUID(),
        ids.tenantB,
      ],
    );

    await owner.query(
      `INSERT INTO finance.series_comprobantes
         (serie, tipo, prefijo, secuencial_actual, ruc_emisor, jurisdiccion, activa, empresa_id)
       VALUES ($1, 'FACTURA', $1, 0, 'RUC-A', 'NI', true, $2),
              ($3, 'FACTURA', $3, 0, 'RUC-B', 'NI', true, $4)
       ON CONFLICT (serie) DO NOTHING`,
      [
        `RLS-A-${ids.tenantA.slice(0, 8)}`,
        ids.tenantA,
        `RLS-B-${ids.tenantB.slice(0, 8)}`,
        ids.tenantB,
      ],
    );
    await owner.query(
      `INSERT INTO finance.comprobantes
         (id, serie, numero, tipo, base_gravada_cents, iva_cents, total_cents, moneda, empresa_id)
       VALUES ($1, $2, '1', 'FACTURA', 100, 15, 115, 'NIO', $3),
              ($4, $5, '1', 'FACTURA', 200, 30, 230, 'NIO', $6)`,
      [
        ids.comprobanteA,
        `RLS-A-${ids.tenantA.slice(0, 8)}`,
        ids.tenantA,
        ids.comprobanteB,
        `RLS-B-${ids.tenantB.slice(0, 8)}`,
        ids.tenantB,
      ],
    );

    await owner.query(
      `INSERT INTO storage.objetos
         (id, tenant_id, bucket, objeto, nombre_original, content_type, tamano_bytes)
       VALUES ($1, $2, 'tenant', $3, 'a.txt', 'text/plain', 10),
              ($4, $5, 'tenant', $6, 'b.txt', 'text/plain', 10)`,
      [
        ids.objetoA,
        ids.tenantA,
        `rls-a-${ids.objetoA}.txt`,
        ids.objetoB,
        ids.tenantB,
        `rls-b-${ids.objetoB}.txt`,
      ],
    );
    await owner.query('COMMIT');
  } catch (error) {
    await owner.query('ROLLBACK');
    throw error;
  }

  try {
    const usuariosA = await filasDe('identity.usuarios', 'tenant_id', ids.tenantA);
    assert.ok(usuariosA.length > 0, 'A debe ver sus usuarios');
    assert.ok(
      usuariosA.every((fila) => fila.tenant_id === ids.tenantA),
      'A no debe ver usuarios de B',
    );

    const pagosA = await filasDe('orders.pagos', 'empresa_id', ids.tenantA);
    assert.ok(pagosA.length > 0, 'A debe ver sus pagos');
    assert.ok(
      pagosA.every((fila) => fila.empresa_id === ids.tenantA),
      'A no debe ver pagos de B',
    );

    const comprobantesA = await filasDe('finance.comprobantes', 'empresa_id', ids.tenantA);
    assert.ok(comprobantesA.length > 0, 'A debe ver sus comprobantes');
    assert.ok(
      comprobantesA.every((fila) => fila.empresa_id === ids.tenantA),
      'A no debe ver comprobantes de B',
    );

    const objetosA = await filasDe('storage.objetos', 'tenant_id', ids.tenantA);
    assert.ok(objetosA.length > 0, 'A debe ver sus objetos');
    assert.ok(
      objetosA.every((fila) => fila.tenant_id === ids.tenantA),
      'A no debe ver objetos de B',
    );

    // Escritura cruzada: 0 filas afectadas, no error de permisos.
    const borrados = await conClaims(
      { sub: ids.userA, rol: 'empresario', tenant_id: ids.tenantA },
      () => app.query('DELETE FROM orders.pagos WHERE id = $1', [ids.pagoB]),
    );
    assert.equal(borrados.rowCount, 0, 'A no debe borrar pagos de B');

    const actualizados = await conClaims(
      { sub: ids.userA, rol: 'empresario', tenant_id: ids.tenantA },
      () =>
        app.query('UPDATE storage.objetos SET nombre_original = $2 WHERE id = $1', [
          ids.objetoB,
          'secuestrado',
        ]),
    );
    assert.equal(actualizados.rowCount, 0, 'A no debe modificar objetos de B');

    // Admin global: ve ambos tenants.
    const admin = await conClaims({ sub: randomUUID(), rol: 'super_admin' }, () =>
      app.query('SELECT tenant_id FROM identity.usuarios WHERE id = ANY($1::uuid[])', [
        [ids.userA, ids.userB],
      ]),
    );
    assert.equal(admin.rows.length, 2, 'super_admin debe ver ambos tenants');
  } finally {
    await owner.query('DELETE FROM storage.objetos WHERE id = ANY($1::uuid[])', [
      [ids.objetoA, ids.objetoB],
    ]);
    await owner.query('DELETE FROM finance.comprobantes WHERE id = ANY($1::uuid[])', [
      [ids.comprobanteA, ids.comprobanteB],
    ]);
    await owner.query('DELETE FROM finance.series_comprobantes WHERE serie LIKE $1', [
      `RLS-%`,
    ]);
    await owner.query('DELETE FROM orders.pagos WHERE id = ANY($1::uuid[])', [
      [ids.pagoA, ids.pagoB],
    ]);
    await owner.query('DELETE FROM identity.usuarios WHERE id = ANY($1::uuid[])', [
      [ids.userA, ids.userB],
    ]);
  }

  console.log('✅ RLS A/B: usuarios, pedidos, finanzas y storage aislados.');
}

main()
  .catch((error) => {
    console.error('❌ RLS A/B:', error.message);
    process.exitCode = 1;
  })
  .finally(async () => {
    await owner.end().catch(() => {});
    await app.end().catch(() => {});
  });
