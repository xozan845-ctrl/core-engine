import 'reflect-metadata';
import { mkdirSync, writeFileSync } from 'fs';
import { resolve } from 'path';
import { NestFactory } from '@nestjs/core';
import { AppModule } from './app.module';
import { crearDocumentoOpenApi } from './swagger';

/**
 * Genera el spec OpenAPI versionado del servicio sin levantar infraestructura:
 * NO se llama a `app.init()`/`listen()`, por lo que los `onModuleInit` de
 * `PgService`/`RabbitService` (que conectan a BD/RabbitMQ) no se ejecutan.
 * Salida: `docs/openapi/logistics-service.json` (lo verifica el gate G-8 en CI).
 */
async function exportar(): Promise<void> {
  const app = await NestFactory.create(AppModule, { logger: false });
  const documento = crearDocumentoOpenApi(app);
  const destino = resolve(__dirname, '../../../docs/openapi');
  mkdirSync(destino, { recursive: true });
  writeFileSync(resolve(destino, 'logistics-service.json'), `${JSON.stringify(documento, null, 2)}\n`);
  await app.close();
}

exportar().catch((err: Error) => {
  console.error(JSON.stringify({ nivel: 'error', msg: 'Fallo el export de OpenAPI', err: err.message }));
  process.exit(1);
});
