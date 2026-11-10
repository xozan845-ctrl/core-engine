import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';

const script = fileURLToPath(new URL('./smoke-remote.mjs', import.meta.url));

function json(res, status, body) {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(JSON.stringify(body));
}

async function run(handler, env = {}) {
  const requests = [];
  const server = createServer(async (req, res) => {
    requests.push(`${req.method} ${req.url}`);
    await handler(req, res);
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const { port } = server.address();

  const child = spawn(process.execPath, [script], {
    env: {
      ...process.env,
      BASE_URL: `http://127.0.0.1:${port}`,
      ADMIN_EMAIL: 'admin@test.local',
      ADMIN_PASSWORD: 'test-only',
      EMPRESARIO_EMAIL: '',
      EMPRESARIO_PASSWORD: '',
      SMOKE_REQUEST_TIMEOUT_MS: '1000',
      ...env,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let output = '';
  child.stdout.on('data', (chunk) => {
    output += chunk;
  });
  child.stderr.on('data', (chunk) => {
    output += chunk;
  });

  try {
    const [code] = await once(child, 'exit');
    return { code, output, requests };
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
}

function happyPath(req, res) {
  if (req.url === '/health') return json(res, 200, { status: 'ok' });
  if (req.url === '/api/v2/auth/login') {
    return json(res, 200, { access_token: 'token' });
  }
  if (req.url.startsWith('/api/v2/catalog/productos')) {
    return json(res, 200, { items: [{ id: 'p1' }], total: 1 });
  }
  return json(res, 404, { codigo: 'NO_ENCONTRADO' });
}

test('smoke-remote: valida salud, login y lectura sin escribir datos', async () => {
  const result = await run(happyPath);

  assert.equal(result.code, 0, result.output);
  assert.match(result.output, /Smoke remoto OK/);
  assert.deepEqual(result.requests, [
    'GET /health',
    'POST /api/v2/auth/login',
    'GET /api/v2/catalog/productos?limite=1',
  ]);
});

test('smoke-remote: falla si el gateway no está sano', async () => {
  const result = await run((req, res) =>
    req.url === '/health' ? json(res, 503, { status: 'degradado' }) : json(res, 200, {}),
  );

  assert.equal(result.code, 1);
  assert.match(result.output, /health → 503/);
});

test('smoke-remote: falla si el login no devuelve token', async () => {
  const result = await run((req, res) => {
    if (req.url === '/health') return json(res, 200, { status: 'ok' });
    if (req.url === '/api/v2/auth/login') return json(res, 401, { codigo: 'CREDENCIALES' });
    return json(res, 200, { items: [{ id: 'p1' }], total: 1 });
  });

  assert.equal(result.code, 1);
  assert.match(result.output, /login → 401/);
});

test('smoke-remote: falla si el catálogo no tiene datos reales', async () => {
  const result = await run((req, res) => {
    if (req.url === '/health') return json(res, 200, { status: 'ok' });
    if (req.url === '/api/v2/auth/login') return json(res, 200, { access_token: 'token' });
    return json(res, 200, { items: [], total: 0 });
  });

  assert.equal(result.code, 1);
  assert.match(result.output, /catálogo está vacío/);
});

test('smoke-remote: exige credenciales del environment', async () => {
  const result = await run(happyPath, { ADMIN_EMAIL: '', ADMIN_PASSWORD: '' });

  assert.equal(result.code, 1);
  assert.match(result.output, /Faltan ADMIN_EMAIL/);
});

test('smoke-remote: verifica ventas de empresa cuando hay credenciales de empresario', async () => {
  const result = await run(
    (req, res) => {
      if (req.url === '/api/v2/empresarios/me/ventas') {
        return json(res, 200, { empresa_id: 'empresa-1', ventas: [{}], liquidaciones: [] });
      }
      return happyPath(req, res);
    },
    {
      EMPRESARIO_EMAIL: 'empresario@test.local',
      EMPRESARIO_PASSWORD: 'test-only',
    },
  );

  assert.equal(result.code, 0, result.output);
  assert.match(result.output, /GET \/api\/v2\/empresarios\/me\/ventas/);
  assert.deepEqual(result.requests, [
    'GET /health',
    'POST /api/v2/auth/login',
    'GET /api/v2/catalog/productos?limite=1',
    'POST /api/v2/auth/login',
    'GET /api/v2/empresarios/me/ventas',
  ]);
});

test('smoke-remote: falla si commissions no responde en stage', async () => {
  const result = await run(
    (req, res) => {
      if (req.url === '/api/v2/empresarios/me/ventas') {
        return json(res, 502, { codigo: 'DESTINO_NO_DISPONIBLE' });
      }
      return happyPath(req, res);
    },
    {
      EMPRESARIO_EMAIL: 'empresario@test.local',
      EMPRESARIO_PASSWORD: 'test-only',
    },
  );

  assert.equal(result.code, 1);
  assert.match(result.output, /ventas de empresa → 502/);
});

test('smoke-remote: exige ambas credenciales de empresario o ninguna', async () => {
  const result = await run(happyPath, {
    EMPRESARIO_EMAIL: 'empresario@test.local',
    EMPRESARIO_PASSWORD: '',
  });

  assert.equal(result.code, 1);
  assert.match(result.output, /EMPRESARIO_EMAIL y EMPRESARIO_PASSWORD/);
});

test('smoke-remote: omite ventas de empresa sin credenciales de empresario', async () => {
  const result = await run(happyPath);

  assert.equal(result.code, 0, result.output);
  assert.match(result.output, /OMITIDO/);
  assert.equal(
    result.requests.some((ruta) => ruta.includes('/empresarios/me/ventas')),
    false,
  );
});
