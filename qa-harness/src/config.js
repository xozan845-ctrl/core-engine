import { readFileSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/**
 * Configuración base del QA Harness.
 * Prioridad: argumentos CLI > variables de entorno > `.env` del repo > defaults
 * no sensibles. Las credenciales (ADMIN_PASSWORD, DATABASE_URL, JWT_SECRET) NO
 * tienen fallback: se validan al usarlas.
 */
export class Configuracion {
  constructor(config) {
    this._private = {};

    // Leer `.env` del repo (las variables de proceso tienen prioridad).
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
    const env = (key) => process.env[key] ?? parsed[key];

    // Exportar el `.env` a process.env para los clientes que lo leen directo
    // (Postgres, RabbitMQ, suites de identidad). No sobrescribe el entorno.
    for (const [key, value] of Object.entries(parsed)) {
      if (process.env[key] === undefined) process.env[key] = value;
    }

    this.baseUrl = config?.baseUrl ?? env('BASE_URL') ?? 'http://localhost:8080';
    this.adminEmail = config?.adminEmail ?? env('ADMIN_EMAIL');
    this.adminPassword = config?.adminPassword ?? env('ADMIN_PASSWORD');
    this.timeoutMs = parseInt(config?.timeoutMs ?? env('QA_TIMEOUT') ?? '30000', 10);
    this.fuzzRounds = parseInt(config?.fuzzRounds ?? env('QA_FUZZ_ROUNDS') ?? '10', 10);
    this.cleanup = config?.cleanup !== false && env('QA_CLEANUP') !== 'false';
    
    // Fix: QA_SUITES can be undefined or string
    const suitesStr = config?.suites ?? env('QA_SUITES') ?? '';
    this.suites = suitesStr ? suitesStr.split(',') : [];
    
    this.format = config?.format ?? env('QA_FORMAT') ?? 'all';
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