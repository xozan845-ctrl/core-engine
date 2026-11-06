import 'reflect-metadata';
import { resolve } from 'path';
import { NestFactory } from '@nestjs/core';
import { exportarDocumentoOpenApi } from '@core/shared';
import { AppModule } from './app.module';
import { crearDocumentoOpenApi } from './swagger';

async function exportar(): Promise<void> {
  const app = await NestFactory.create(AppModule, { logger: false });
  const documento = crearDocumentoOpenApi(app);
  exportarDocumentoOpenApi(documento, 'scheduling-service', resolve(__dirname, '../../../docs/openapi'));
  await app.close();
}

exportar().catch((err: Error) => {
  console.error(JSON.stringify({ nivel: 'error', msg: 'Fallo el export de OpenAPI', err: err.message }));
  process.exit(1);
});
