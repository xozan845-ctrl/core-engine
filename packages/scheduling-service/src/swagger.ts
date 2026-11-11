import type { INestApplication } from '@nestjs/common';
import { DocumentBuilder, SwaggerModule } from '@nestjs/swagger';
import type { OpenAPIObject } from '@nestjs/swagger';
import { envolverDocumentoOpenApi } from '@core/shared';

export function crearDocumentoOpenApi(app: INestApplication): OpenAPIObject {
  const config = new DocumentBuilder()
    .setTitle('Core Engine · scheduling-service')
    .setDescription('Agenda multi-vertical: recursos, disponibilidad, citas y recordatorios')
    .setVersion('v1')
    .addBearerAuth()
    .build();
  return envolverDocumentoOpenApi(
    SwaggerModule.createDocument(app, config),
  ) as unknown as OpenAPIObject;
}
