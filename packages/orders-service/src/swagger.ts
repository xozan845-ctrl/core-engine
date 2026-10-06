import type { INestApplication } from '@nestjs/common';
import { DocumentBuilder, SwaggerModule } from '@nestjs/swagger';
import type { OpenAPIObject } from '@nestjs/swagger';

/**
 * Construye el documento OpenAPI del servicio. Fuente de verdad del contrato
 * de la API (R-DO-6) y compartido por `main.ts` (sirve `/docs`) y el script
 * `swagger:export` (genera el spec versionado).
 */
export function crearDocumentoOpenApi(app: INestApplication): OpenAPIObject {
  const config = new DocumentBuilder()
    .setTitle('Core Engine · orders-service')
    .setDescription('Órdenes y transacciones: pedidos, carrito y comisiones (CQRS + Event Sourcing, Tabla 21)')
    .setVersion('v1')
    .addBearerAuth()
    .build();
  return SwaggerModule.createDocument(app, config);
}
