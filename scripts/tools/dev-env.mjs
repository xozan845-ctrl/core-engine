#!/usr/bin/env node
/**
 * dev-env.mjs — genera `.env` de desarrollo con secretos aleatorios (R-DS-1).
 *
 * Uso:
 *   npm run env:dev            # crea .env a partir de .env.example (falla si existe)
 *   npm run env:dev -- --force # sobrescribe .env
 *   npm run env:dev -- --rotate # rota solo los secretos de aplicación
 *
 * En una instalación nueva rota también las credenciales de Postgres, RabbitMQ
 * y MinIO. `--rotate` NO toca esas tres porque los volúmenes ya están
 * inicializados con las anteriores. No imprime secretos y deja el archivo 600.
 */
import { chmodSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
import { randomBytes } from 'node:crypto';
import { join, resolve } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';

const APP = new Set(['JWT_SECRET', 'INTERNAL_API_KEY', 'ADMIN_PASSWORD']);
const DATOS = new Set([
  'POSTGRES_PASSWORD',
  'APP_DB_PASSWORD',
  'RABBITMQ_PASSWORD',
  'MINIO_ROOT_PASSWORD',
]);
const TODOS = new Set([...APP, ...DATOS]);

function nuevosValores() {
  return {
    JWT_SECRET: randomBytes(48).toString('hex'),
    INTERNAL_API_KEY: randomBytes(32).toString('hex'),
    // >= 12 caracteres y variedad de clases para el seed del administrador.
    ADMIN_PASSWORD: `${randomBytes(18).toString('base64url')}aA1!`,
    POSTGRES_PASSWORD: randomBytes(24).toString('hex'),
    APP_DB_PASSWORD: randomBytes(24).toString('hex'),
    RABBITMQ_PASSWORD: randomBytes(24).toString('hex'),
    MINIO_ROOT_PASSWORD: randomBytes(24).toString('hex'),
  };
}

function parsear(contenido) {
  const lineas = contenido.split('\n');
  const valores = {};
  for (const linea of lineas) {
    const match = /^([A-Z][A-Z0-9_]*)=(.*)$/.exec(linea);
    if (match) valores[match[1]] = match[2];
  }
  return { lineas, valores };
}

function aplicar(contenido, overrides, incluirDatos) {
  const { lineas, valores } = parsear(contenido);
  const permitidos = incluirDatos ? TODOS : APP;
  for (const clave of Object.keys(overrides)) {
    if (permitidos.has(clave)) valores[clave] = overrides[clave];
  }

  // Reconstruir las URLs compuestas para que no queden con la credencial vieja.
  valores.POSTGRES_URL = `postgres://${valores.POSTGRES_USER}:${valores.POSTGRES_PASSWORD}@${valores.POSTGRES_HOST}:${valores.POSTGRES_PORT}/${valores.POSTGRES_DB}`;
  if (valores.APP_DB_PASSWORD) {
    valores.DATABASE_URL = `postgres://core_engine_app:${valores.APP_DB_PASSWORD}@${valores.POSTGRES_HOST}:${valores.POSTGRES_PORT}/${valores.POSTGRES_DB}`;
  }
  valores.RABBITMQ_URL = `amqp://${valores.RABBITMQ_USER}:${valores.RABBITMQ_PASSWORD}@rabbitmq:5672`;

  const reconstruidas = new Set(['POSTGRES_URL', 'RABBITMQ_URL']);
  if (valores.APP_DB_PASSWORD) reconstruidas.add('DATABASE_URL');
  return lineas
    .map((linea) => {
      const match = /^([A-Z][A-Z0-9_]*)=(.*)$/.exec(linea);
      if (!match) return linea;
      const clave = match[1];
      if (!permitidos.has(clave) && !reconstruidas.has(clave)) return linea;
      return `${clave}=${valores[clave]}`;
    })
    .join('\n');
}

function escribir(ruta, contenido) {
  writeFileSync(ruta, contenido, { encoding: 'utf8', mode: 0o600 });
  chmodSync(ruta, 0o600);
}

function main() {
  const argv = process.argv.slice(2);
  const args = new Set(argv);
  const ejemplo = join(repoRoot, '.env.example');
  const outIndex = argv.indexOf('--out');
  const destino =
    outIndex !== -1 && argv[outIndex + 1] ? resolve(argv[outIndex + 1]) : join(repoRoot, '.env');
  const prueba = outIndex !== -1;

  if (!existsSync(ejemplo)) throw new Error('No existe .env.example.');

  const valores = nuevosValores();

  if (args.has('--rotate')) {
    if (!existsSync(destino)) throw new Error('No existe .env; usa `npm run env:dev`.');
    escribir(destino, aplicar(readFileSync(destino, 'utf8'), valores, false));
    console.log('✅ Secretos de aplicación rotados en .env (JWT, clave interna, admin).');
    console.log('   Las credenciales de Postgres/RabbitMQ/MinIO no se tocan (volúmenes existentes).');
    console.log('   Reinicia el stack y actualiza la contraseña del admin en la base si ya existía.');
    return;
  }

  if (existsSync(destino) && !args.has('--force') && !prueba) {
    throw new Error('.env ya existe. Usa `--force` para sobrescribirlo o `--rotate` para rotar secretos.');
  }

  escribir(destino, aplicar(readFileSync(ejemplo, 'utf8'), valores, true));
  console.log('✅ .env creado con secretos aleatorios (modo 600).');
  console.log('   Levanta el stack con: docker compose -f docker-compose.dev.yml up');
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
