#!/usr/bin/env node
/**
 * smoke-remote.mjs — smoke de solo lectura para stage/producción (R-CD-12).
 *
 * Uso:
 *   BASE_URL=https://stage.example \
 *   ADMIN_EMAIL=... ADMIN_PASSWORD=... \
 *   EMPRESARIO_EMAIL=... EMPRESARIO_PASSWORD=... \
 *   EDGE_BASIC_AUTH=usuario:clave \
 *   npm run smoke:remote
 *
 * `EDGE_BASIC_AUTH` solo es necesario en entornos con gate de borde (staging).
 * `EMPRESARIO_EMAIL`/`EMPRESARIO_PASSWORD` son opcionales: si se configuran,
 * el smoke verifica además `GET /api/v1/empresarios/me/ventas` (ADR-37).
 *
 * No crea usuarios, productos, órdenes ni pagos: solo comprueba salud del
 * gateway, login y lecturas reales. El flujo E2E completo sigue siendo
 * `npm run demo` en desarrollo local.
 */
const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8080';
const ADMIN_EMAIL = process.env.ADMIN_EMAIL;
const ADMIN_PASSWORD = process.env.ADMIN_PASSWORD;
const EMPRESARIO_EMAIL = process.env.EMPRESARIO_EMAIL;
const EMPRESARIO_PASSWORD = process.env.EMPRESARIO_PASSWORD;
const EDGE_BASIC_AUTH = process.env.EDGE_BASIC_AUTH;
const REQUEST_TIMEOUT_MS = Number(process.env.SMOKE_REQUEST_TIMEOUT_MS ?? 10_000);

if (!Number.isInteger(REQUEST_TIMEOUT_MS) || REQUEST_TIMEOUT_MS < 1) {
  throw new Error('SMOKE_REQUEST_TIMEOUT_MS debe ser un entero positivo');
}
if (!ADMIN_EMAIL || !ADMIN_PASSWORD) {
  throw new Error(
    'Faltan ADMIN_EMAIL y/o ADMIN_PASSWORD: configúralos como secretos del environment (R-CD-12).',
  );
}
if (Boolean(EMPRESARIO_EMAIL) !== Boolean(EMPRESARIO_PASSWORD)) {
  throw new Error(
    'EMPRESARIO_EMAIL y EMPRESARIO_PASSWORD deben configurarse juntos para verificar ventas de empresa (R-CD-12).',
  );
}

let pasoActual = 0;

function paso(nombre) {
  pasoActual += 1;
  process.stdout.write(`\n[${pasoActual}] ${nombre} ... `);
}

async function request(method, path, { token, body } = {}) {
  const attempts = method === 'GET' ? 2 : 1;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      const response = await fetch(`${BASE_URL}${path}`, {
        method,
        headers: {
          'content-type': 'application/json',
          // Con sesion se usa Bearer (el borde valida el JWT); antes del login
          // se usa el Basic del gate de staging, si esta configurado.
          ...(token
            ? { authorization: `Bearer ${token}` }
            : EDGE_BASIC_AUTH
              ? { authorization: `Basic ${Buffer.from(EDGE_BASIC_AUTH).toString('base64')}` }
              : {}),
        },
        body: body ? JSON.stringify(body) : undefined,
        signal: globalThis.AbortSignal.timeout(REQUEST_TIMEOUT_MS),
      });
      const text = await response.text();
      let json;
      try {
        json = text ? JSON.parse(text) : null;
      } catch {
        json = text;
      }
      return { status: response.status, json, headers: response.headers };
    } catch (error) {
      if (attempt === attempts - 1) {
        throw new Error(
          `${method} ${path}: sin respuesta tras ${attempts} intento(s) de ${REQUEST_TIMEOUT_MS} ms`,
          { cause: error },
        );
      }
    }
  }
}

function assertStatus(result, expected, label) {
  if (result.status !== expected) {
    throw new Error(`${label} → ${result.status}: ${JSON.stringify(result.json)}`);
  }
}

async function login(correo, contrasena, etiqueta) {
  const result = await request('POST', '/api/v1/auth/login', {
    body: { correo, contrasena },
  });
  assertStatus(result, 200, etiqueta);
  if (!result.json?.access_token) {
    throw new Error(`${etiqueta} no devolvió access_token`);
  }
  return result.json.access_token;
}

async function main() {
  paso('GET /health (gateway en pie, R-CD-12)');
  const health = await request('GET', '/health');
  assertStatus(health, 200, 'health');
  console.log('OK');

  paso('Login de administrador (credenciales del environment)');
  const token = await login(ADMIN_EMAIL, ADMIN_PASSWORD, 'login');
  console.log('OK');

  paso('GET /api/v1/catalog/productos (lectura real autenticada)');
  const catalog = await request('GET', '/api/v1/catalog/productos?limite=1', { token });
  assertStatus(catalog, 200, 'catálogo');
  if (!Array.isArray(catalog.json?.items)) {
    throw new Error('el catálogo no devolvió una lista `items`');
  }
  if (catalog.json.items.length === 0) {
    throw new Error('el catálogo está vacío: no hay datos reales que verificar');
  }
  console.log(`OK (${catalog.json.items.length} producto(s), total=${catalog.json.total ?? '?'})`);

  if (EMPRESARIO_EMAIL && EMPRESARIO_PASSWORD) {
    paso('Login de empresario (credenciales del environment)');
    const tokenEmpresario = await login(
      EMPRESARIO_EMAIL,
      EMPRESARIO_PASSWORD,
      'login empresario',
    );
    console.log('OK');

    paso('GET /api/v1/empresarios/me/ventas (commissions-service en stage, ADR-37)');
    const ventas = await request('GET', '/api/v1/empresarios/me/ventas', {
      token: tokenEmpresario,
    });
    assertStatus(ventas, 200, 'ventas de empresa');
    if (
      typeof ventas.json?.empresa_id !== 'string' ||
      !Array.isArray(ventas.json?.ventas) ||
      !Array.isArray(ventas.json?.liquidaciones)
    ) {
      throw new Error(
        'ventas de empresa no devolvió { empresa_id, ventas[], liquidaciones[] }',
      );
    }
    console.log(
      `OK (${ventas.json.ventas.length} venta(s), ${ventas.json.liquidaciones.length} liquidación(es))`,
    );
  } else {
    paso(
      'GET /api/v1/empresarios/me/ventas (omitido: faltan EMPRESARIO_EMAIL/EMPRESARIO_PASSWORD)',
    );
    console.log('OMITIDO');
  }

  console.log('\n✅ Smoke remoto OK: salud, login y lecturas reales verificados (sin escrituras).');
}

try {
  await main();
} catch (error) {
  console.error(`\n❌ ${error.message}`);
  process.exit(1);
}
