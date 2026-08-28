/**
 * Suite 17 — Preflight de Esquema (regresión de infra/db/init)
 *
 * Afirma que todas las tablas/vistas/outbox críticas existen y que
 * `orders.eventos` tiene el esquema RICH (no el legado). Es la red de
 * seguridad automatizada para los bugs de init SQL corregidos en esta
 * sesión (disparador CREATE TRIGGER IF NOT EXISTS, columna timestamp en
 * field, y la colisión de órdenes.eventos).
 */
import { ClientoPostgres } from '../utils.js';

const ESPERADAS = {
  catalog: [
    'ajustes_stock', 'eventos_procesados', 'historico_precios', 'inventario_mensual_mv',
    'outbox', 'producto_catalogo_vista', 'productos', 'reservas_ordenes', 'stock_vista',
  ],
  commissions: [
    'comisiones', 'compensaciones_devoluciones', 'eventos_procesados', 'liquidaciones', 'outbox', 'pagos',
  ],
  field: [
    'asistencia', 'clientes', 'cumplimiento', 'eventos_procesados', 'incidencias', 'outbox',
    'paradas', 'pedidos', 'personal', 'rutas', 'tracking', 'vehiculos', 'visitas',
  ],
  finance: [
    'asiento_detalles', 'asientos', 'asientos_vista', 'comprobantes', 'declaraciones',
    'eventos_procesados', 'jurisdicciones', 'libro_mayor_mv', 'metricas_mensuales',
    'metricas_vendedores', 'outbox', 'plan_cuentas', 'proyecciones', 'regimenes_fiscales',
    'series_comprobantes', 'sujetos_fiscales',
  ],
  identity: ['outbox', 'usuarios'],
  logistics: ['envios', 'eventos_procesados', 'outbox'],
  orders: [
    'carritos', 'comisiones_vista', 'eventos', 'eventos_procesados', 'orden_timeline',
    'orden_vista', 'outbox', 'proyeccion_ordenes',
  ],
  stores: ['eventos_procesados', 'historico_precios', 'ofertas', 'outbox', 'tiendas'],
};

// orders.eventos: esquema RICH esperado (el legado causaba el aborto del init)
const ORDERS_EVENTOS_RICH = ['aggregate_id', 'aggregate_type', 'tipo', 'payload', 'metadata', 'version', 'creado_en'];
const ORDERS_EVENTOS_LEGACY = ['order_id', 'datos', 'ocurrido_en'];

export async function suite17Esquema(pg, cfg) {
  const errores = [];
  const resultados = [];

  const fallar = (test, detalle) => {
    resultados.push({ test, status: 'FAIL', pass: false });
    errores.push(`${test} :: ${detalle}`);
  };
  const ok = (test, cond, detalle = '') => {
    resultados.push({ test, status: cond ? 'OK' : 'FAIL', pass: !!cond });
    if (!cond) errores.push(`${test} :: ${detalle}`);
  };

  try {
    // Relaciones existentes = tablas + vistas + vistas materializadas
    // (varias "vistas" CQRS se crean como TABLE/MATVIEW según el init).
    const filas = await pg.query(
      `SELECT schemaname, tablename AS nombre FROM pg_tables
       WHERE schemaname NOT IN ('pg_catalog','information_schema','pg_toast')
       UNION ALL
       SELECT schemaname, viewname AS nombre FROM pg_views
       WHERE schemaname NOT IN ('pg_catalog','information_schema')
       UNION ALL
       SELECT schemaname, matviewname AS nombre FROM pg_matviews
       WHERE schemaname NOT IN ('pg_catalog','information_schema')`,
    );
    const existentes = new Set(filas.map((r) => `${r.schemaname}.${r.nombre}`));

    for (const [schema, tablas] of Object.entries(ESPERADAS)) {
      for (const t of tablas) {
        ok(`Tabla ${schema}.${t} existe`, existentes.has(`${schema}.${t}`), 'no encontrada');
      }
    }

    // Vistas materializadas / vistas CQRS críticas (tablas + views + matviews)
    for (const v of [
      'catalog.producto_catalogo_vista', 'catalog.stock_vista',
      'finance.asientos_vista', 'finance.libro_mayor_mv', 'catalog.inventario_mensual_mv',
    ]) {
      ok(`Vista ${v} existe`, existentes.has(v), 'no encontrada');
    }

    // orders.eventos: esquema RICH
    const cols = await pg.query(
      `SELECT column_name FROM information_schema.columns
       WHERE table_schema='orders' AND table_name='eventos'`,
    );
    const colSet = new Set(cols.map((c) => c.column_name));
    for (const c of ORDERS_EVENTOS_RICH) {
      ok(`orders.eventos columna ${c}`, colSet.has(c), 'columna faltante');
    }
    for (const c of ORDERS_EVENTOS_LEGACY) {
      ok(`orders.eventos NO tiene columna legada ${c}`, !colSet.has(c), 'columna legada presente (colisión)');
    }

    // Triggers de proyección financiera (el bug CREATE TRIGGER IF NOT EXISTS)
    const trig = await pg.query(
      `SELECT trigger_name FROM information_schema.triggers
       WHERE trigger_schema='finance'
         AND trigger_name IN ('asientos_proyeccion_trigger','detalles_proyeccion_trigger')`,
    );
    const trigSet = new Set(trig.map((t) => t.trigger_name));
    for (const tn of ['asientos_proyeccion_trigger', 'detalles_proyeccion_trigger']) {
      ok(`Trigger finance.${tn} existe`, trigSet.has(tn), 'trigger faltante');
    }

    // Trigger de event-sourcing de órdenes (01d)
    const trigO = await pg.query(
      `SELECT trigger_name FROM information_schema.triggers
       WHERE trigger_schema='orders' AND trigger_name='eventos_proyeccion_trigger'`,
    );
    ok('Trigger orders.eventos_proyeccion_trigger existe', (trigO[0]?.trigger_name) === 'eventos_proyeccion_trigger', 'trigger faltante');

    // Todos los esquemas de negocio tienen su outbox (patrón Outbox para CQRS)
    const sinOutbox = Object.keys(ESPERADAS).filter((s) => !existentes.has(`${s}.outbox`));
    ok('Cada esquema de dominio tiene outbox', sinOutbox.length === 0, `faltan: ${sinOutbox.join(', ')}`);
  } catch (e) {
    fallar('Suite 17 ejecución', e.message);
  }

  const pasados = resultados.filter((r) => r.pass).length;
  return {
    test: 'Preflight de Esquema (regresión infra/db/init)',
    pass: errores.length === 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}
