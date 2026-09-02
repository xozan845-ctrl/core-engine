import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);

// ── Cliente HTTP (fetch nativo, sin dependencias) ──────────────
export class ClientoHTTP {
  constructor(base) {
    this.base = base ?? 'http://localhost:8080';
    this.headers = { 'Content-Type': 'application/json', 'Accept': 'application/json' };
  }

  async get(path, headers) {
    const url = `${this.base}${path}`;
    const res = await fetch(url, { method: 'GET', headers: { ...this.headers, ...headers } });
    const text = await res.text();
    let responseBody;
    try { responseBody = JSON.parse(text); } catch { responseBody = text; }
    return { status: res.status, body: responseBody, headers: Object.fromEntries(res.headers.entries()) };
  }

  async post(path, body, headers) {
    const url = `${this.base}${path}`;
    const h = { 'Content-Type': 'application/json', ...this.headers, ...(headers || {}) };
    const res = await fetch(url, { method: 'POST', headers: h, body: body ? JSON.stringify(body) : undefined });
    const text = await res.text();
    let responseBody;
    try { responseBody = JSON.parse(text); } catch { responseBody = text; }
    return { status: res.status, body: responseBody, headers: Object.fromEntries(res.headers.entries()) };
  }

  async patch(path, body, headers) {
    const url = `${this.base}${path}`;
    const h = { 'Content-Type': 'application/json', ...this.headers, ...(headers || {}) };
    const res = await fetch(url, { method: 'PATCH', headers: h, body: JSON.stringify(body) });
    const text = await res.text();
    let responseBody;
    try { responseBody = JSON.parse(text); } catch { responseBody = text; }
    return { status: res.status, body: responseBody, headers: Object.fromEntries(res.headers.entries()) };
  }

  async delete(path, headers) {
    const url = `${this.base}${path}`;
    const h = { 'Content-Type': 'application/json', ...this.headers, ...(headers || {}) };
    const res = await fetch(url, { method: 'DELETE', headers: h });
    const text = await res.text();
    let responseBody;
    try { responseBody = JSON.parse(text); } catch { responseBody = text; }
    return { status: res.status, body: responseBody, headers: Object.fromEntries(res.headers.entries()) };
  }

  async put(path, body, headers) {
    const url = `${this.base}${path}`;
    const h = { 'Content-Type': 'application/json', ...this.headers, ...(headers || {}) };
    const res = await fetch(url, { method: 'PUT', headers: h, body: JSON.stringify(body) });
    const text = await res.text();
    let responseBody;
    try { responseBody = JSON.parse(text); } catch { responseBody = text; }
    return { status: res.status, body: responseBody, headers: Object.fromEntries(res.headers.entries()) };
  }

  async request(method, path, body, headers) {
    const url = `${this.base}${path}`;
    const h = { 'Content-Type': 'application/json', ...this.headers, ...(headers || {}) };
    const res = await fetch(url, { method, headers: h, body: body ? JSON.stringify(body) : undefined });
    const text = await res.text();
    let responseBody;
    try { responseBody = JSON.parse(text); } catch { responseBody = text; }
    return { status: res.status, body: responseBody, headers: Object.fromEntries(res.headers.entries()) };
  }
}

// ── Cliente RabbitMQ ──────────────────────────────────────────────
export class ClientoRabbitMQ {
  constructor(base = 'http://localhost:15672', user = 'core-engine', pass = '0b66c112e3ceca4e3b6a648219c88c60') {
    this.base = base;
    this.basic = 'Basic ' + Buffer.from(`${user}:${pass}`).toString('base64');
    this.colasDlq = [
      'catalog.pedidos.dlq', 'commissions.liquidacion.dlq', 'commissions.pedidos.dlq',
      'finance.pedidos.dlq', 'logistics.pagos.dlq', 'orders.resultados.dlq', 'stores.stock.dlq',
    ];
    this.colas = [
      'catalog.pedidos', 'commissions.liquidacion', 'commissions.pedidos',
      'finance.pedidos', 'logistics.pagos', 'orders.resultados', 'stores.stock',
    ];
  }

  async request(method, path, body) {
    const url = `${this.base}${path}`;
    const res = await fetch(url, {
      method,
      headers: {
        authorization: this.basic,
        accept: 'application/json',
        'Content-Type': 'application/json',
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
    const text = await res.text();
    let responseBody;
    try { responseBody = JSON.parse(text); } catch { responseBody = text; }
    return { status: res.status, body: responseBody, headers: Object.fromEntries(res.headers.entries()) };
  }

  async getQueueInfo(queue) {
    return this.request('GET', `/api/queues/%2f/${encodeURIComponent(queue)}`);
  }

  async getQueueDepth(queue) {
    const info = await this.getQueueInfo(queue);
    return info?.message_count ?? 0;
  }

  async getDLQDepth() {
    let total = 0;
    for (const q of this.colasDlq) {
      const info = await this.getQueueInfo(q);
      total += info?.message_count ?? 0;
    }
    return total;
  }

  async getRabbitMQInfo() {
    return this.request('GET', '/api/overview');
  }

  /** Publica via management API: payload = body en formato string. */
  async publish(exchange, routingKey, message) {
    const res = await this.request('POST', '/api/exchanges/%2f/publish', {
      exchange,
      routing_key: routingKey,
      properties: { message_id: crypto.randomUUID(), content_type: 'application/json' },
      payload: JSON.stringify(message),
      mandatory: false,
    });
    return { ok: res.body?.routed === true, status: res.status };
  }

  async publishExchanges(exchange, routingKey, message) {
    const res = await this.request('POST', `/api/exchanges/${encodeURIComponent(exchange)}/publish`, {
      exchange,
      routing_key: routingKey,
      properties: { message_id: crypto.randomUUID() },
      payload: JSON.stringify(message),
    });
    return { ok: res.body?.routed === true, status: res.status };
  }
}

// ── Cliente Postgres ──────────────────────────────────────────────────
export class ClientoPostgres {
  constructor() {
    // Use localhost from host, docker internal from containers
    this.url = process.env.DATABASE_URL ?? 'postgres://core_engine:88cf331f0740655a65ac11877f1dc619@localhost:5432/core_engine';
    this.pool = null;
  }

  getPool() {
    if (!this.pool) {
      const { Pool } = require('pg');
      this.pool = new Pool({ connectionString: this.url, max: 10 });
    }
    return this.pool;
  }

  async query(text, params = []) {
    const pool = this.getPool();
    const result = await pool.query(text, params);
    return result.rows;
  }

  async queryOne(text, params = []) {
    const result = await this.query(text, params);
    return result[0] ?? null;
  }

  async transaccion(fn) {
    const pool = this.getPool();
    const client = await pool.connect();
    try {
      await client.query('BEGIN');
      const result = await fn(client);
      await client.query('COMMIT');
      return result;
    } catch (err) {
      await client.query('ROLLBACK');
      throw err;
    } finally {
      client.release();
    }
  }
}

// ── Token Manager ──────────────────────────────────────────────────────
export class TokenManager {
  constructor(secret, accessTtl, refreshTtl) {
    this.secret = secret ?? process.env.JWT_SECRET ?? 'dev_secret';
    this.accessTtl = accessTtl ?? '900s';
    this.refreshTtl = refreshTtl ?? '7d';
  }

  async generarToken(payload) {
    const jwt = require('jsonwebtoken');
    return jwt.sign(payload, this.secret, { expiresIn: this.accessTtl, algorithm: 'HS256' });
  }

  async verificarToken(token) {
    const jwt = require('jsonwebtoken');
    return jwt.verify(token, this.secret, { algorithms: ['HS256'] });
  }

  async refreshToken(refreshToken) {
    const jwt = require('jsonwebtoken');
    const payload = jwt.verify(refreshToken, this.secret, { algorithms: ['HS256'] });
    return this.generarToken(payload);
  }

  async validarRefresh(refreshToken) {
    try {
      return await this.refreshToken(refreshToken);
    } catch {
      throw new Error('Token de refresco inválido');
    }
  }
}

// ── Utilities ────────────────────────────────────────────────────────────
export function verificarContrasena(contrasena, minimo = 8) {
  if (!contrasena || contrasena.length < minimo) return false;
  return /^[a-zA-Z0-9!@#$%^&*()_+\-=\[\]{}|;:',.<>?/~\s]+$/.test(contrasena);
}

export function generarId() {
  return crypto.randomUUID().replace(/-/g, '').substring(0, 16);
}

export function obtenerTiempo() {
  return new Date().toISOString();
}

export function truncarTexto(texto, maxLen = 500) {
  return texto.length > maxLen ? texto.substring(0, maxLen) + '...' : texto;
}