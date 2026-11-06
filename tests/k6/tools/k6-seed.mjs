#!/usr/bin/env node
/**
 * k6-seed.mjs — datos de carga para el fixture local de k6.
 *
 * Usa el tenant `Flow Co %` creado por el flujo de prueba (empresario/tienda/
 * oferta) y añade 100 productos + ofertas con SKU `K6-*`. Es idempotente.
 *
 * Uso:
 *   npm run k6:seed
 *
 * No se ejecuta en CI: stage/prod usan datos reales y las variables K6_*.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import pg from 'pg';
import { repoRoot } from '../../../scripts/engine/lib/paths.mjs';

const env = Object.fromEntries(
  readFileSync(join(repoRoot, '.env'), 'utf8')
    .split('\n')
    .filter((linea) => linea && !linea.startsWith('#') && linea.includes('='))
    .map((linea) => {
      const i = linea.indexOf('=');
      return [linea.slice(0, i), linea.slice(i + 1)];
    }),
);

if (!env.POSTGRES_USER || !env.POSTGRES_PASSWORD || !env.POSTGRES_DB) {
  console.error('❌ Faltan POSTGRES_USER/PASSWORD/DB en .env.');
  process.exit(1);
}
// El seed corre en el host: `postgres` solo resuelve dentro de la red Docker.
const url = `postgres://${encodeURIComponent(env.POSTGRES_USER)}:${encodeURIComponent(
  env.POSTGRES_PASSWORD,
)}@127.0.0.1:${env.POSTGRES_PORT || '5432'}/${env.POSTGRES_DB}`;

const pool = new pg.Pool({ connectionString: url });
try {
  const empresa = (
    await pool.query(
      `SELECT id FROM identity.empresas WHERE nombre LIKE 'Flow Co %' ORDER BY creado_en DESC LIMIT 1`,
    )
  ).rows[0];
  if (!empresa) throw new Error('No hay tenant Flow Co; ejecuta primero el flujo de prueba.');

  const tienda = (
    await pool.query(
      `SELECT id FROM stores.tiendas WHERE empresa_id=$1 ORDER BY creado_en DESC LIMIT 1`,
      [empresa.id],
    )
  ).rows[0];
  if (!tienda) throw new Error('El tenant Flow Co no tiene tienda.');

  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    await client.query(
      `INSERT INTO catalog.productos
         (empresa_id, sku, nombre, categoria, precio_base_cents, stock, estado)
       SELECT $1, 'K6-' || g, 'K6 Product ' || g, 'k6', 10000 + g * 100, 100, 'disponible'
       FROM generate_series(1, 100) AS g
       ON CONFLICT (empresa_id, sku) DO NOTHING`,
      [empresa.id],
    );
    await client.query(
      `INSERT INTO stores.ofertas
         (tienda_id, producto_id, sku, producto_nombre, margen,
          precio_base_cents, precio_venta_cents, stock, estado)
       SELECT $1, p.id, p.sku, p.nombre, 20, p.precio_base_cents,
              (p.precio_base_cents * 1.2)::int, p.stock, 'activa'
       FROM catalog.productos p
       WHERE p.empresa_id=$2 AND p.sku LIKE 'K6-%'
       ON CONFLICT (tienda_id, producto_id) DO NOTHING`,
      [tienda.id, empresa.id],
    );
    await client.query('COMMIT');
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    client.release();
  }

  const { rows } = await pool.query(
    `SELECT
       (SELECT count(*) FROM catalog.productos WHERE empresa_id=$1 AND sku LIKE 'K6-%') AS productos,
       (SELECT count(*) FROM stores.ofertas WHERE tienda_id=$2) AS ofertas`,
    [empresa.id, tienda.id],
  );
  console.log(`✅ Fixture k6: ${rows[0].productos} productos K6 y ${rows[0].ofertas} ofertas en la tienda.`);
} finally {
  await pool.end();
}
