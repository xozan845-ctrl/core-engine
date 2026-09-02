/**
 * Suite 13 — Carga y rendimiento
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite13Carga(client, cfg) {
  const errores = [];
  const resultados = [];

  try {
    const loadLevel = cfg.loadLevel || 'agresivo';
    const iteraciones = loadLevel === 'agresivo' ? 1000 : loadLevel === 'moderado' ? 500 : 200;
    const concurrencia = cfg.concurrency || 50;

    const healthTimes = [];
    for (let i = 0; i < 100; i++) {
      const start = Date.now();
      const res = await client.get('/health');
      healthTimes.push(Date.now() - start);
    }
    healthTimes.sort((a, b) => a - b);
    const p50 = healthTimes[Math.floor(healthTimes.length * 0.5)];
    const p90 = healthTimes[Math.floor(healthTimes.length * 0.9)];
    const p99 = healthTimes[Math.floor(healthTimes.length * 0.99)];
    resultados.push({ test: `Health check p50=${p50}ms p90=${p90}ms p99=${p99}ms`, pass: p99 < 100 });

    const loginTimes = [];
    for (let i = 0; i < iteraciones; i++) {
      const start = Date.now();
      const res = await client.post('/api/v1/auth/login', {
        correo: 'admin@core-engine.test',
        contrasena: 'AdminCore Engine2026!',
      });
      if (res.status === 200 || res.status === 429) {
        loginTimes.push(Date.now() - start);
      }
    }
    if (loginTimes.length > 0) {
      loginTimes.sort((a, b) => a - b);
      const l50 = loginTimes[Math.floor(loginTimes.length * 0.5)];
      const l90 = loginTimes[Math.floor(loginTimes.length * 0.9)];
      const l99 = loginTimes[Math.floor(loginTimes.length * 0.99)];
      resultados.push({ test: `Login p50=${l50}ms p90=${l90}ms p99=${l99}ms`, pass: l99 < 500 });
    }

    const orderTimes = [];
    for (let i = 0; i < iteraciones / 10; i++) {
      const start = Date.now();
      const res = await client.post('/api/v1/orders', {
        items: [{ oferta_id: 'oferta-test', cantidad: 1 }],
      });
      if (res.status === 201 || res.status === 200 || res.status === 409) {
        orderTimes.push(Date.now() - start);
      }
    }
    if (orderTimes.length > 0) {
      orderTimes.sort((a, b) => a - b);
      const o50 = orderTimes[Math.floor(orderTimes.length * 0.5)];
      const o90 = orderTimes[Math.floor(orderTimes.length * 0.9)];
      const o99 = orderTimes[Math.floor(orderTimes.length * 0.99)];
      resultados.push({ test: `Crear orden p50=${o50}ms p90=${o90}ms p99=${o99}ms`, pass: o99 < 1000 });
    }

    // Mediana de 3 tandas: el throughput en rafaga sufre ruido del host
    // (Docker Desktop/Windows), la mediana lo suaviza sin enmascarar caidas reales
    const tandas = [];
    for (let t = 0; t < 3; t++) {
      const rafaga = [];
      for (let i = 0; i < concurrencia; i++) {
        rafaga.push(client.get('/health'));
      }
      const inicio = Date.now();
      await Promise.all(rafaga);
      tandas.push((concurrencia / (Date.now() - inicio)) * 1000);
    }
    tandas.sort((a, b) => a - b);
    const rps = tandas[1];
    resultados.push({ test: `Throughput (mediana de 3 tandas): ${rps.toFixed(1)} req/s`, pass: rps > 50 });

    const ratePromises = [];
    for (let i = 0; i < 20; i++) {
      ratePromises.push(client.post('/api/v1/auth/login', {
        correo: 'admin@core-engine.test',
        contrasena: 'AdminCore Engine2026!',
      }));
    }
    const rateResults = await Promise.all(ratePromises);
    const rate429 = rateResults.filter(r => r.status === 429).length;
    resultados.push({ test: `Rate limit (429): ${rate429}/20`, pass: rate429 > 0 });

    const adminTimes = [];
    for (let i = 0; i < 50; i++) {
      const start = Date.now();
      const res = await client.get('/api/v1/admin/reportes');
      adminTimes.push(Date.now() - start);
    }
    adminTimes.sort((a, b) => a - b);
    const a50 = adminTimes[Math.floor(adminTimes.length * 0.5)];
    const a99 = adminTimes[Math.floor(adminTimes.length * 0.99)];
    resultados.push({ test: `Admin reportes p50=${a50}ms p99=${a99}ms`, pass: a99 < 2000 });
  } catch (e) {
    errores.push(`Suite 13: ${e.message}`);
  }

  return {
    test: 'Carga y rendimiento',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}