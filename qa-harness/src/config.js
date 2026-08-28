import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/**
 * Configuración base del QA Harness.
 * Lee .env.example por defecto y permite sobreescribir vía variables de entorno.
 */
export class Configuracion {
  constructor(config) {
    this._private = {};

    // Leer .env.example o .env si existe
    const envPath = join(__dirname, '../../.env');
    const envContent = existsSync(envPath) ? readFileSync(envPath, 'utf-8') : '';
    const parsed = {};
    for (const line of envContent.split('\n')) {
      const trimmed = line.trim();
      if (trimmed && !trimmed.startsWith('#')) {
        const [key, ...val] = trimmed.split('=');
        const valStr = val.join('=').trim();
        if (valStr) parsed[key] = valStr;
      }
    }

    this.baseUrl = config?.baseUrl ?? parsed?.BASE_URL ?? 'http://localhost:8080';
    this.adminEmail = config?.adminEmail ?? parsed?.ADMIN_EMAIL ?? 'admin@bodegahub.test';
    this.adminPassword = config?.adminPassword ?? parsed?.ADMIN_PASSWORD ?? 'AdminBodegaHub2026!';
    this.timeoutMs = parseInt(config?.timeoutMs ?? parsed?.QA_TIMEOUT ?? '30000', 10);
    this.concurrency = parseInt(config?.concurrency ?? parsed?.QA_CONCURRENCY ?? '50', 10);
    this.fuzzRounds = parseInt(config?.fuzzRounds ?? parsed?.QA_FUZZ_ROUNDS ?? '10', 10);
    this.loadLevel = config?.loadLevel ?? parsed?.QA_LOAD_LEVEL ?? 'agresivo';
    this.cleanup = config?.cleanup !== false && parsed?.QA_CLEANUP !== 'false';
    
    // Fix: QA_SUITES can be undefined or string
    const suitesStr = config?.suites ?? parsed?.QA_SUITES ?? '';
    this.suites = suitesStr ? suitesStr.split(',') : [];
    
    this.format = config?.format ?? parsed?.QA_FORMAT ?? 'all';
  }

  get(key) {
    return this._private[key] ?? this[key];
  }

  set(key, value) {
    this._private[key] = value;
  }

  getFull() {
    return { ...this._private, ...this };
  }
}