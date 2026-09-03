/**
 * Suite 21 — Market Intelligence (endpoints reales)
 * Rutas validadas contra market-intelligence-service:
 * resumen, tendencias, rendimiento, mapa-calor, cobertura,
 * forecast, anomalias, score, calidad.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite21MarketIntelligence(client, cfg, adminToken) {
  const errores = [];
  const resultados = [];

  if (!adminToken) {
    errores.push('No admin token provided');
    return { test: 'Market Intelligence, Analytics Predictivo y Geoespacial', pass: false, errores, resultados, timestamp: new Date().toISOString() };
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
    // Fase 1 & 2: Descriptivo y Vistas Materializadas
    await probar('Resumen Ejecutivo', 'GET', '/api/v1/inteligencia/resumen');
    await probar('Tendencias Históricas', 'GET', '/api/v1/inteligencia/tendencias?rango=mes');
    await probar('Rendimiento Vendedores', 'GET', '/api/v1/inteligencia/rendimiento');

    // Geoespacial (PostGIS)
    await probar('Mapa de Calor', 'GET', '/api/v1/inteligencia/mapa-calor?rango=semana');
    await probar('Cobertura Zonal', 'GET', '/api/v1/inteligencia/cobertura?lat=12.148&lng=-86.273&radio_km=5');

    // Fase 3: Analítica Predictiva y Feature Store
    await probar('Forecast de Demanda', 'GET', '/api/v1/inteligencia/forecast/demanda?sku=TEST&dias=7');
    await probar('Detección de Anomalías', 'GET', '/api/v1/inteligencia/anomalias?limite=10');
    
    // Asumiendo UUID genérico para vendedor (el servicio controla que exista o responde con "Sin datos")
    const vendedorId = '00000000-0000-0000-0000-000000000000';
    await probar('Score de Churn Vendedor', 'GET', `/api/v1/inteligencia/vendedores/${vendedorId}/score`);

    // Endpoints personales del vendedor (pueden devolver 200 o 403 dependiendo de cómo el gateway maneja 'me')
    const miMapa = await client.get('/api/v1/inteligencia/me/mapa-calor', auth);
    resultados.push({ test: 'Mi Mapa de Calor (admin → 200/403)', status: miMapa.status, pass: [200, 403, 404].includes(miMapa.status) });

    const miScore = await client.get('/api/v1/inteligencia/me/score', auth);
    resultados.push({ test: 'Mi Score Predictivo (admin → 200/403)', status: miScore.status, pass: [200, 403, 404].includes(miScore.status) });

    // Fase 4: Observabilidad y Data Quality
    await probar('Dashboard de Calidad de Datos', 'GET', '/api/v1/inteligencia/calidad?horas=24');

  } catch (e) {
    errores.push(`Suite 21: ${e.message}`);
  }

  return {
    test: 'Market Intelligence, Analytics Predictivo y Geoespacial',
    pass: resultados.filter(r => r.pass).length === resultados.length, // Opcionalmente flexibilizar
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}
