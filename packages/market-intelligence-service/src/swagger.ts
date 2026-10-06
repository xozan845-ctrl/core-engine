import type { INestApplication } from '@nestjs/common';
import { DocumentBuilder, SwaggerModule } from '@nestjs/swagger';
import type { OpenAPIObject } from '@nestjs/swagger';

/**
 * Construye el documento OpenAPI del servicio. Fuente de verdad del contrato
 * de la API (R-DO-6) y compartido por `main.ts` (sirve `/docs`) y el script
 * `swagger:export` (genera el spec versionado).
 *
 * NOTA: este servicio está fuera de producción por ahora (docker-compose.extra.yml,
 * ADR-17); el contrato se documenta igualmente.
 */
export function crearDocumentoOpenApi(app: INestApplication): OpenAPIObject {
  const config = new DocumentBuilder()
    .setTitle('Core Engine · market-intelligence-service')
    .setDescription('Inteligencia de mercado: métricas, mapa de calor y predicción (fuera de producción)')
    .setVersion('v1')
    .addBearerAuth()
    .build();
  return SwaggerModule.createDocument(app, config);
}
