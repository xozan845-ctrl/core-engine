import 'reflect-metadata';
import { resolve } from 'path';
import { NestFactory } from '@nestjs/core';
import { AppModule } from './app.module';
import { exportarDocumentoOpenApi } from '@core/shared';
import { crearDocumentoOpenApi } from './swagger';

/**
 * Genera el spec OpenAPI versionado del servicio sin levantar infraestructura:
 * NO se llama a `app.init()`/`listen()`, por lo que los `onModuleInit` de
 * `PgService`/`RabbitService` (que conectan a BD/RabbitMQ) no se ejecutan.
 * Salida: `docs/openapi/storage-service/public.openapi.json` e `internal.openapi.json`
 * (las verifica el gate G-8 en CI).
 */
async function exportar(): Promise<void> {
  const app = await NestFactory.create(AppModule, { logger: false });
  const documento = crearDocumentoOpenApi(app);
  const destino = resolve(__dirname, '../../../docs/openapi');
  exportarDocumentoOpenApi(documento, 'storage-service', destino);
  await app.close();
}

exportar().catch((err: Error) => {
  console.error(JSON.stringify({ nivel: 'error', msg: 'Fallo el export de OpenAPI', err: err.message }));
  process.exit(1);
});
