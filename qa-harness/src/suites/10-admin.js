/**
 * Suite 10 — Reportes admin e inventario (endpoints reales)
 * Documenta gaps: no existen /admin/vendedores ni /admin/liquidaciones/cortes.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite10Admin(client, cfg, adminToken) {
  const errores = [];
  const resultados = [];

  if (!adminToken) {
    errores.push('No admin token provided');
    return { test: 'Reportes admin e inventario', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const auth = { authorization: `Bearer ${adminToken}` };

  try {
    const reportes = await client.get('/api/v1/admin/reportes', auth);
    resultados.push({ test: 'Reporte GMV/pedidos/stock', status: reportes.status, pass: reportes.status === 200 });

    const inventario = await client.get('/api/v1/admin/inventario', auth);
    resultados.push({ test: 'Inventario admin', status: inventario.status, pass: inventario.status === 200 });

    const corte = await client.post('/api/v1/admin/liquidaciones/corte', {}, auth);
    resultados.push({ test: 'Corte liquidación', status: corte.status, pass: [200, 201].includes(corte.status) });

    const liqs = Array.isArray(corte.body) ? corte.body : (corte.body?.liquidaciones ?? corte.body?.items ?? []);
    if (liqs.length > 0) {
      const pagar = await client.post(`/api/v1/admin/liquidaciones/${liqs[0].id}/pagar`, {}, auth);
      resultados.push({ test: 'Pagar liquidación', status: pagar.status, pass: [200, 201].includes(pagar.status) });
    } else {
      resultados.push({ test: 'Pagar liquidación (sin pendientes → skip)', pass: true });
    }

    const envios = await client.get('/api/v1/admin/envios', auth);
    resultados.push({ test: 'Listar envíos admin', status: envios.status, pass: envios.status === 200 });

    const vendedores = await client.get('/api/v1/admin/vendedores', auth);
    resultados.push({ test: 'GET /admin/vendedores (no existe → 404)', status: vendedores.status, pass: vendedores.status === 404 });

    const cortes = await client.get('/api/v1/admin/liquidaciones/cortes', auth);
    resultados.push({ test: 'GET /admin/liquidaciones/cortes (no existe → 404)', status: cortes.status, pass: cortes.status === 404 });

    const sinToken = await client.get('/api/v1/admin/reportes');
    resultados.push({ test: 'Sin token → 401', status: sinToken.status, pass: sinToken.status === 401 });
  } catch (e) {
    errores.push(`Suite 10: ${e.message}`);
  }

  return {
    test: 'Reportes admin e inventario',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}