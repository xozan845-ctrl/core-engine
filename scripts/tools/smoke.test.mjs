import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';
import { corteLiquidacionesValido } from './smoke-contract.mjs';

const cli = fileURLToPath(new URL('../cli.mjs', import.meta.url));

test('el corte quincenal acepta 201 con lista, como declara OpenAPI', () => {
  assert.equal(corteLiquidacionesValido({ status: 201, json: [{ monto_cents: 13800 }] }), true);
  assert.equal(corteLiquidacionesValido({ status: 201, json: [] }), true);
  assert.equal(corteLiquidacionesValido({ status: 200, json: [] }), false);
  assert.equal(corteLiquidacionesValido({ status: 201, json: { monto_cents: 13800 } }), false);
});

function json(res, status, body) {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(JSON.stringify(body));
}

async function ejecutarSmoke(handler, timeoutMs = 1_000, waitTimeoutMs = 30_000) {
  const requests = [];
  const server = createServer(async (req, res) => {
    requests.push(req.url);
    await handler(req, res);
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const address = server.address();
  const child = spawn(process.execPath, [cli, 'smoke'], {
    env: {
      ...process.env,
      BASE_URL: `http://127.0.0.1:${address.port}`,
      ADMIN_EMAIL: 'admin@test.local',
      ADMIN_PASSWORD: 'test-only',
      SMOKE_REQUEST_TIMEOUT_MS: String(timeoutMs),
      SMOKE_WAIT_TIMEOUT_MS: String(waitTimeoutMs),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let output = '';
  child.stdout.on('data', (chunk) => { output += chunk; });
  child.stderr.on('data', (chunk) => { output += chunk; });
  const watchdog = setTimeout(() => child.kill('SIGKILL'), 10_000);
  try {
    const [code, signal] = await once(child, 'exit');
    return { code, signal, output, requests };
  } finally {
    clearTimeout(watchdog);
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
}

test('smoke falla de inmediato si el empresario no puede crear al vendedor', async () => {
  const result = await ejecutarSmoke(async (req, res) => {
    if (req.url === '/ready') return json(res, 200, { status: 'ok', servicios: {} });
    if (req.url === '/api/v1/auth/login') return json(res, 200, { access_token: 'admin' });
    if (req.url === '/api/v1/auth/registro') {
      let body = '';
      for await (const chunk of req) body += chunk;
      const rol = JSON.parse(body).rol;
      return rol === 'empresario'
        ? json(res, 201, { access_token: 'empresario' })
        : json(res, 201, { access_token: 'comprador' });
    }
    if (req.url === '/api/v1/empresas' && req.method === 'POST') {
      return json(res, 201, { id: 'empresa-1' });
    }
    if (req.url === '/api/v1/empresas/empresa-1/vendedores' && req.method === 'POST') {
      return json(res, 500, { codigo: 'FALLO' });
    }
    return json(res, 500, { codigo: 'RUTA_INESPERADA' });
  });

  assert.equal(result.code, 1);
  assert.match(result.output, /crear vendedor → 500/);
  assert.equal(result.requests.some((ruta) => ruta.startsWith('/api/v1/catalog')), false);
});

test('smoke reintenta una sola vez un GET cuando se corta la red', async () => {
  const result = await ejecutarSmoke((req) => req.socket.destroy());

  assert.equal(result.code, 1);
  assert.equal(result.signal, null);
  assert.deepEqual(result.requests, ['/ready', '/ready']);
  assert.match(result.output, /GET \/ready: sin respuesta tras 2 intento\(s\)/);
});

test('smoke corta una petición que excede su timeout', async () => {
  const result = await ejecutarSmoke(() => {}, 150);

  assert.equal(result.code, 1);
  assert.equal(result.signal, null);
  assert.match(result.output, /GET \/ready: sin respuesta tras 2 intento\(s\) de 150 ms/);
});

test('smoke no repite un POST si la respuesta se pierde', async () => {
  const result = await ejecutarSmoke((req, res) => {
    if (req.url === '/ready') return json(res, 200, { status: 'ok', servicios: {} });
    req.socket.destroy();
  });

  assert.equal(result.code, 1);
  assert.deepEqual(result.requests, ['/ready', '/api/v1/auth/login']);
  assert.match(result.output, /POST \/api\/v1\/auth\/login: sin respuesta tras 1 intento\(s\)/);
});

test('smoke limita la espera de la saga asíncrona por paso', async () => {
  const result = await ejecutarSmoke(async (req, res) => {
    const ruta = req.url;
    if (ruta === '/ready') return json(res, 200, { status: 'ok', servicios: {} });
    if (ruta === '/api/v1/auth/login') return json(res, 200, { access_token: 'admin' });
    if (ruta === '/api/v1/auth/registro') return json(res, 201, { access_token: 'usuario' });
    if (ruta === '/api/v1/empresas' && req.method === 'POST') {
      return json(res, 201, { id: 'empresa-1' });
    }
    if (ruta === '/api/v1/empresas/empresa-1/vendedores' && req.method === 'POST') {
      return json(res, 201, { id: 'vendedor-1' });
    }
    if (ruta === '/api/v1/catalog/productos' && req.method === 'POST') {
      return json(res, 201, { id: 'producto-1', sku: 'SKU-1' });
    }
    if (ruta === '/api/v1/admin/reportes') return json(res, 401, { codigo: 'NO_AUTORIZADO' });
    if (ruta === '/api/v1/vendedores/tienda' && req.method === 'POST') {
      return json(res, 201, { id: 'tienda-1' });
    }
    if (ruta === '/api/v1/vendedores/productos') {
      return json(res, 201, { id: 'oferta-1', precio_venta: '1150.00' });
    }
    if (ruta === '/api/v1/tiendas/tienda-1') return json(res, 200, { ofertas: [{}] });
    if (ruta === '/api/v1/orders' && req.method === 'POST') {
      return json(res, 201, { id: 'orden-1' });
    }
    if (ruta === '/api/v1/orders/orden-1') return json(res, 200, { estado: 'pendiente' });
    return json(res, 500, { codigo: 'RUTA_INESPERADA' });
  }, 1_000, 200);

  assert.equal(result.code, 1);
  assert.match(result.output, /Expiró la espera de 200 ms: orden pagada/);
  assert.equal(result.requests.some((ruta) => ruta.includes('/estado')), false);
});
