import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Listas de precio y escalas por cantidad (Fase P1.1).
 * Verifica la resolucion real contra PostgreSQL: lista explicita, segmento,
 * lista predeterminada, escala base y fallback al precio del item.
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

process.env.DATABASE_URL = connectionString;

const MIGRACIONES = [
  '01_esquemas.sql',
  '01c_cqrs_catalog.sql',
  '01e_adr25_empresas.sql',
  '01f_slugs.sql',
  '01h_catalogo_por_empresa.sql',
  '01i_variantes_imagenes.sql',
  '11_catalogo_productos_servicios.sql',
  '12_catalogo_operaciones_avanzadas.sql',
  '25_catalog_items.sql',
  '26_stores_items.sql',
  '42a_catalog_items_vista_reset.sql',
  '43_catalog_item_model.sql',
  '44_catalog_items_metadatos.sql',
  '48_catalog_retiro_legacy.sql',
];

let client;
let pgService;
let precios;
const empresaId = randomUUID();
const itemId = randomUUID();
let listaMayorista;
let listaVip;

async function aplicar(nombre) {
  const sql = await readFile(new URL(`../../infra/db/init/${nombre}`, import.meta.url), 'utf8');
  await client.query(sql);
}

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    // Las politicas RLS usan auth.rol(); en una BD vacia el esquema auth aun
    // no existe porque las suites aplican solo la cadena del catalogo.
    await client.query(`
      CREATE SCHEMA IF NOT EXISTS auth;
      CREATE OR REPLACE FUNCTION auth.jwt() RETURNS jsonb LANGUAGE sql STABLE AS $$
        SELECT COALESCE(NULLIF(current_setting('request.jwt.claims', true), ''), '{}')::jsonb
      $$;
      CREATE OR REPLACE FUNCTION auth.rol() RETURNS text LANGUAGE sql STABLE AS $$
        SELECT COALESCE(auth.jwt() ->> 'rol', 'anon')
      $$;
      DO $$ BEGIN
        IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN CREATE ROLE anon NOLOGIN; END IF;
        IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN CREATE ROLE authenticated NOLOGIN; END IF;
      END $$;
    `);
    const { rows: [{ final }] } = await client.query(
      `SELECT to_regclass('catalog.items') IS NOT NULL
              AND to_regclass('catalog.productos') IS NULL AS final`,
    );
    if (!final) {
      const { rows: [{ existe }] } = await client.query(
        `SELECT to_regnamespace('identity') IS NOT NULL AS existe`,
      );
      const migraciones = existe
        ? MIGRACIONES.filter((nombre) => nombre !== '01_esquemas.sql')
        : MIGRACIONES;
      for (const nombre of migraciones) await aplicar(nombre);
    }
    await aplicar('64_listas_precio_escalas.sql');
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const [{ PgService }, { PreciosService }] = await Promise.all([
    import('@core/shared'),
    import('../../packages/catalog-service/dist/items/precios.service.js'),
  ]);
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
  precios = new PreciosService(pgService);
});

after(async () => {
  await client.query('DELETE FROM catalog.listas_precio WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await client.query('DELETE FROM catalog.items WHERE empresa_id = $1', [empresaId]).catch(() => undefined);
  await pgService?.onModuleDestroy();
  await client?.end();
});

test('resuelve precios por lista, segmento, escala base y fallback', { concurrency: false }, async () => {
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    await client.query(
      `INSERT INTO catalog.items (id, empresa_id, tipo, codigo, nombre, precio_cents)
       VALUES ($1, $2, 'producto', 'ESC-01', 'Item con escala', 10000)`,
      [itemId, empresaId],
    );

    const base = await precios.crearPrecio(empresaId, null, {
      item_id: itemId,
      desde_cantidad: 10,
      precio_cents: 8000,
    });
    assert.equal(base.lista_id, null);
    const item = await client.query('SELECT tipo_precio FROM catalog.items WHERE id = $1', [itemId]);
    assert.equal(item.rows[0].tipo_precio, 'escala');

    const escalaBase = await precios.resolver(empresaId, itemId, 10);
    assert.equal(escalaBase.origen, 'escala');
    assert.equal(escalaBase.precio_unitario_cents, 8000);
    assert.equal(escalaBase.precio_total_cents, 80000);

    listaMayorista = await precios.crearLista(empresaId, {
      codigo: 'mayorista',
      nombre: 'Mayorista',
      segmento_codigo: 'mayorista',
      predeterminada: true,
    });
    listaVip = await precios.crearLista(empresaId, {
      codigo: 'vip',
      nombre: 'VIP',
      segmento_codigo: 'vip',
    });
    assert.notEqual(listaMayorista.id, listaVip.id);

    await precios.crearPrecio(empresaId, listaMayorista.id, {
      item_id: itemId,
      desde_cantidad: 5,
      precio_cents: 7000,
    });
    await precios.crearPrecio(empresaId, listaVip.id, {
      item_id: itemId,
      desde_cantidad: 1,
      hasta_cantidad: 4,
      precio_cents: 6000,
    });

    await assert.rejects(
      precios.crearPrecio(empresaId, null, {
        item_id: itemId,
        desde_cantidad: 15,
        precio_cents: 7500,
      }),
      (error) => error?.codigo === 'CONFLICTO',
    );

    const uno = await precios.resolver(empresaId, itemId, 1);
    assert.equal(uno.origen, 'base');
    assert.equal(uno.precio_unitario_cents, 10000);

    const diez = await precios.resolver(empresaId, itemId, 10);
    assert.equal(diez.origen, 'lista');
    assert.equal(diez.precio_unitario_cents, 7000);
    assert.equal(diez.precio_total_cents, 70000);

    const mayorista = await precios.resolver(empresaId, itemId, 12, {
      lista_codigo: 'mayorista',
    });
    assert.equal(mayorista.origen, 'lista');
    assert.equal(mayorista.precio_unitario_cents, 7000);

    const vip = await precios.resolver(empresaId, itemId, 3, { segmento_codigo: 'vip' });
    assert.equal(vip.origen, 'lista');
    assert.equal(vip.precio_unitario_cents, 6000);

    const vipSinRango = await precios.resolver(empresaId, itemId, 5, { segmento_codigo: 'vip' });
    assert.equal(vipSinRango.origen, 'base');
    assert.equal(vipSinRango.precio_unitario_cents, 10000);

    const escalas = await precios.listarEscalas(empresaId, itemId);
    assert.deepEqual(escalas.map((fila) => fila.precio_cents), [8000]);
    await precios.eliminarPrecio(empresaId, null, base.id);
    const itemFinal = await client.query('SELECT tipo_precio FROM catalog.items WHERE id = $1', [itemId]);
    assert.equal(itemFinal.rows[0].tipo_precio, 'fijo');
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }
});
