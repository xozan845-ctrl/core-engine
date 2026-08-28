/**
 * Suite 00 — Preflight (verificación pre-ejecución)
 */
import { ClientoHTTP, ClientoRabbitMQ, ClientoPostgres } from '../utils.js';
import { Configuracion } from '../config.js';

export async function preflight(config) {
  const errores = [];
  const servicios = {};
  const configuracion = config.getFull();

  try {
    const res = await new ClientoHTTP().get('/health');
    if (res.status === 200 && res.body?.api_gateway === 'ok') {
      servicios.gateway = 'online';
    } else {
      errores.push(`Health de API Gateway: ${res.status} ${JSON.stringify(res.body)}`);
    }
  } catch (e) {
    errores.push(`Health: ${e.message}`);
  }

  try {
    const pgRes = await new ClientoPostgres().queryOne('SELECT 1');
    if (pgRes) {
      servicios.postgres = 'online';
    } else {
      errores.push('Postgres no disponible');
    }
  } catch (e) {
    errores.push(`Postgres: ${e.message}`);
  }

  try {
    const rabbitMQ = new ClientoRabbitMQ();
    const rabbitInfo = await rabbitMQ.getRabbitMQInfo();
    servicios.rabbitMQ = rabbitInfo?.status === 'running' ? 'online' : 'no_responding';
  } catch (e) {
    errores.push(`RabbitMQ: ${e.message}`);
  }

  const endpoints = [
    '/api/v1/auth/login',
    '/api/v1/catalog/productos',
    '/api/v1/orders',
    '/api/v1/admin/reportes',
    '/health',
    '/metrics',
  ];
  for (const ep of endpoints) {
    try {
      const res = await new ClientoHTTP().get(ep);
      if (res.status >= 200 && res.status < 300) {
        servicios[ep] = 'ok';
      } else {
        errores.push(`Endpoint ${ep}: ${res.status}`);
      }
    } catch (e) {
      errores.push(`Endpoint ${ep}: ${e.message}`);
    }
  }

  return {
    estado: errores.length === 0 ? 'ok' : 'fallo',
    servicios,
    configuracion,
    errores,
    timestamp: new Date().toISOString(),
  };
}
