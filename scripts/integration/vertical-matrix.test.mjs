import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test, { after, before } from 'node:test';
import pg from 'pg';

/**
 * Matriz multi-vertical (ADR-0001/R-I-12): el mismo código de alta debe
 * producir tenants distintos según el pack, sin ramas por rubro.
 *
 * No se comparan los packs contra sí mismos: el contrato esperado se fija
 * aquí y se contrasta contra PostgreSQL real después de crear cada empresa.
 */

const connectionString = process.env.TEST_DATABASE_URL;
if (!connectionString) throw new Error('TEST_DATABASE_URL requerida para la BD efimera');
const databaseUrl = new URL(connectionString);
if (databaseUrl.pathname !== '/core_engine_test' || !['localhost', '127.0.0.1'].includes(databaseUrl.hostname)) {
  throw new Error('La suite solo acepta core_engine_test en localhost');
}

process.env.DATABASE_URL = connectionString;
process.env.STORAGE_SERVICE_URL = 'http://127.0.0.1:9';

const MIGRACIONES = [
  '01_esquemas.sql',
  '01c_cqrs_catalog.sql',
  '01e_adr25_empresas.sql',
  '01f_slugs.sql',
  '01h_catalogo_por_empresa.sql',
  '13_empresa_metadatos.sql',
  '16_media_background_plataforma.sql',
  '17_media_background_empresa.sql',
  '19_personalizacion_tienda.sql',
  '22_herencia_tema.sql',
  '24_multi_vertical.sql',
  '29_vertical_packs.sql',
];

const CASOS = [
  {
    vertical: 'retail',
    pack: 'pack_retail',
    tipo: 'comercial',
    modalidad: 'plataforma',
    activas: [
      'catalogo_items',
      'codigo_barras',
      'facturacion',
      'fulfillment_delivery',
      'fulfillment_pickup',
      'inventario_stock',
      'liquidaciones',
    ],
    prohibidas: ['agenda_recursos', 'inventario_lotes', 'logistica_rutas'],
  },
  {
    vertical: 'farmacia',
    pack: 'pack_farmacia',
    tipo: 'comercial',
    modalidad: 'plataforma',
    activas: [
      'catalogo_items',
      'codigo_barras',
      'facturacion',
      'inventario_lotes',
      'inventario_stock',
      'liquidaciones',
    ],
    prohibidas: ['agenda_recursos', 'logistica_rutas'],
  },
  {
    vertical: 'belleza',
    pack: 'pack_belleza',
    tipo: 'comercial',
    modalidad: 'plataforma',
    activas: ['agenda_recursos', 'catalogo_items', 'facturacion', 'liquidaciones'],
    prohibidas: ['inventario_stock', 'inventario_lotes', 'logistica_rutas'],
  },
  {
    vertical: 'dental',
    pack: 'pack_dental',
    tipo: 'comercial',
    modalidad: 'plataforma',
    activas: [
      'agenda_recursos',
      'catalogo_items',
      'cotizaciones',
      'facturacion',
      'liquidaciones',
    ],
    prohibidas: ['inventario_stock', 'inventario_lotes', 'logistica_rutas'],
  },
  {
    vertical: 'flota',
    pack: 'pack_flota',
    tipo: 'logistica',
    modalidad: 'propia',
    activas: [
      'catalogo_items',
      'cotizaciones',
      'facturacion',
      'fulfillment_delivery',
      'fulfillment_onsite',
      'fulfillment_pickup',
      'liquidaciones',
      'logistica_rutas',
    ],
    prohibidas: ['agenda_recursos', 'inventario_stock', 'inventario_lotes'],
  },
];

let client;
let pgService;
let empresas;

before(async () => {
  client = new pg.Client({ connectionString });
  await client.connect();
  await client.query('SELECT pg_advisory_lock(38194201)');
  try {
    const { rows: [{ existe }] } = await client.query(
      `SELECT to_regnamespace('identity') IS NOT NULL AS existe`,
    );
    const migraciones = existe ? MIGRACIONES.filter((name) => name !== '01_esquemas.sql') : MIGRACIONES;
    for (const name of migraciones) {
      const sql = await readFile(new URL(`../../infra/db/init/${name}`, import.meta.url), 'utf8');
      await client.query(sql);
    }
  } finally {
    await client.query('SELECT pg_advisory_unlock(38194201)');
  }

  const [{ PgService }, { EmpresasService }] = await Promise.all([
    import('@core/shared'),
    import('../../packages/identity-service/dist/empresas/empresas.service.js'),
  ]);
  pgService = new PgService({ url: connectionString });
  await pgService.onModuleInit();
  empresas = new EmpresasService(pgService, {});
});

after(async () => {
  await pgService?.onModuleDestroy();
  await client?.end();
});

async function crearUsuario() {
  const usuarioId = randomUUID();
  await client.query(
    `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
     VALUES ($1, 'Empresario matriz', $2, 'hash', 'empresario')`,
    [usuarioId, `matriz-${usuarioId}@test.dev`],
  );
  return usuarioId;
}

async function limpiar(empresaId, usuarioId) {
  await client.query('DELETE FROM identity.empresa_capacidades WHERE empresa_id = $1', [empresaId]);
  await client.query('DELETE FROM identity.empresa_miembros WHERE empresa_id = $1', [empresaId]);
  await client.query('DELETE FROM identity.empresas WHERE id = $1', [empresaId]);
  await client.query('DELETE FROM identity.usuarios WHERE id = $1', [usuarioId]);
}

for (const caso of CASOS) {
  test(`tenant ${caso.vertical}: nace del ${caso.pack} con sus capacidades activas`, async () => {
    const usuarioId = await crearUsuario();
    let empresaId;
    try {
      const empresa = await empresas.crear(
        usuarioId,
        `Tenant ${caso.vertical} ${randomUUID().slice(0, 8)}`,
        caso.modalidad,
        undefined,
        undefined,
        caso.tipo,
        caso.vertical,
        caso.pack,
      );
      empresaId = empresa.id;

      assert.equal(empresa.vertical_id, caso.vertical);
      assert.equal(empresa.pack_codigo, caso.pack);
      assert.equal(empresa.pack_version, 1);

      const { rows: capacidades } = await client.query(
        `SELECT capacidad_codigo
           FROM identity.empresa_capacidades
          WHERE empresa_id = $1 AND activo = true
          ORDER BY capacidad_codigo`,
        [empresa.id],
      );
      assert.deepEqual(
        capacidades.map((fila) => fila.capacidad_codigo),
        caso.activas,
        `${caso.vertical} debe activar exactamente las capacidades de su pack`,
      );

      for (const prohibida of caso.prohibidas) {
        assert.ok(
          !caso.activas.includes(prohibida),
          `${caso.vertical} no debe activar ${prohibida}`,
        );
      }
    } finally {
      if (empresaId) await limpiar(empresaId, usuarioId);
      else await client.query('DELETE FROM identity.usuarios WHERE id = $1', [usuarioId]);
    }
  });
}

test('rechaza un pack que no corresponde a la vertical solicitada', async () => {
  await assert.rejects(
    empresas.crear(
      randomUUID(),
      `Tenant incompatible ${randomUUID().slice(0, 8)}`,
      'plataforma',
      undefined,
      undefined,
      'comercial',
      'retail',
      'pack_belleza',
    ),
    (error) => error?.codigo === 'PACK_VERTICAL_INCOMPATIBLE',
  );
});
