/**
 * Suite 09 — Finanzas, contabilidad y fiscalidad (endpoints reales)
 * Rutas validadas contra finance-service: cuentas, asientos, libros, proyecciones,
 * kpis, punto-equilibrio, tablero, regimenes, jurisdicciones, sujetos, declaraciones, comprobantes.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite09Finanzas(client, cfg, adminToken) {
  const errores = [];
  const resultados = [];

  if (!adminToken) {
    errores.push('No admin token provided');
    return { test: 'Finanzas, contabilidad, régimen fiscal NIC', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const auth = { authorization: `Bearer ${adminToken}` };

  const probar = async (test, method, path, body) => {
    const r = method === 'POST'
      ? await client.post(path, body, auth)
      : await client.get(path, auth);
    resultados.push({ test, status: r.status, pass: r.status === 200 || r.status === 201 });
    return r;
  };

  try {
    await probar('Plan de cuentas', 'GET', '/api/v1/finanzas/cuentas');
    await probar('Asientos contables', 'GET', '/api/v1/finanzas/asientos');

    const asiento = await client.post('/api/v1/finanzas/asientos', {
      tipo: 'MANUAL',
      concepto: 'Pruesta partida doble',
      moneda: 'C$',
      detalles: [
        { cuenta_codigo: '1.1.1', debe_cents: 10000, haber_cents: 0, concepto: 'Caja' },
        { cuenta_codigo: '4.1', debe_cents: 0, haber_cents: 10000, concepto: 'Ingresos' },
      ],
    }, auth);
    resultados.push({ test: 'Crear asiento (partida doble)', status: asiento.status, pass: [200, 201].includes(asiento.status) });

    await probar('Libro mayor', 'GET', '/api/v1/finanzas/libro-mayor/1.1.1');
    await probar('Libro de ventas', 'GET', '/api/v1/finanzas/libro-ventas');
    await probar('Libro de compras', 'GET', '/api/v1/finanzas/libro-compras');
    await probar('Proyecciones', 'GET', '/api/v1/finanzas/proyecciones');
    await probar('Punto de equilibrio', 'GET', '/api/v1/finanzas/punto-equilibrio');
    await probar('KPIs financieros', 'GET', '/api/v1/finanzas/kpis');
    await probar('Tablero financiero', 'GET', '/api/v1/finanzas/tablero');
    await probar('Regímenes fiscales', 'GET', '/api/v1/finanzas/regimenes');
    await probar('Jurisdicciones', 'GET', '/api/v1/finanzas/jurisdicciones');
    await probar('Sujetos fiscales', 'GET', '/api/v1/finanzas/sujetos');
    await probar('Declaraciones', 'GET', '/api/v1/finanzas/declaraciones');
    await probar('Comprobantes fiscales', 'GET', '/api/v1/finanzas/comprobantes');

    // el vendedor consulta su situacion fiscal via /vendedores/me/fiscal
    const vFiscal = await client.get('/api/v1/vendedores/me/fiscal', auth);
    resultados.push({ test: 'Fiscal del vendedor (admin → 403 o 200)', status: vFiscal.status, pass: [200, 403].includes(vFiscal.status) });
  } catch (e) {
    errores.push(`Suite 09: ${e.message}`);
  }

  return {
    test: 'Finanzas, contabilidad, régimen fiscal NIC',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}