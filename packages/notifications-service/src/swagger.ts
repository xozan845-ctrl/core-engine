import type { INestApplication } from '@nestjs/common';
import { DocumentBuilder, SwaggerModule } from '@nestjs/swagger';
import type { OpenAPIObject } from '@nestjs/swagger';

/**
 * Documento OpenAPI del servicio. No expone rutas públicas de negocio: solo
 * documenta la superficie interna (vacía por ahora) para que el agregador
 * valide la taxonomía de ambos contratos (R-DO-8).
 */
export function crearDocumentoOpenApi(app: INestApplication): OpenAPIObject {
  const config = new DocumentBuilder()
    .setTitle('Core Engine · notifications-service')
    .setDescription('Notificaciones transaccionales por correo (ADR-0003)')
    .setVersion('v1')
    .addApiKey({ type: 'apiKey', in: 'header', name: 'x-internal-key' }, 'internalKey')
    .build();
  return SwaggerModule.createDocument(app, config);
}
