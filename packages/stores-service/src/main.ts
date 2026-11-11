import 'reflect-metadata';
import { NestFactory } from '@nestjs/core';
import { SwaggerModule } from '@nestjs/swagger';
import type { Request, Response } from 'express';
import { AppModule } from './app.module';
import { crearDocumentoOpenApi } from './swagger';
import {
  MetricsService,
  TrazabilidadInterceptor,
  EnvelopeInterceptor,
  DtoValidationPipe,
  DomainErrorFilter,
  NOMBRE_SERVICIOS,
  PUERTOS,
  Logger,
  RolesGuard,
  CapabilityGuard,
} from '@core/shared';

async function bootstrap(): Promise<void> {
  const app = await NestFactory.create(AppModule, { logger: false });
  app.useGlobalPipes(new DtoValidationPipe());
  app.useGlobalInterceptors(
    new TrazabilidadInterceptor(app.get(MetricsService), NOMBRE_SERVICIOS.STORES),
    new EnvelopeInterceptor(),
  );
  app.useGlobalFilters(new DomainErrorFilter());
  app.useGlobalGuards(new RolesGuard());
  app.useGlobalGuards(new CapabilityGuard());
  const puerto = Number(process.env.PORT ?? PUERTOS.STORES);
  const expressApp = app.getHttpAdapter().getInstance();
  const metrics = app.get(MetricsService);
  expressApp.get('/metrics', async (_req: Request, res: Response) =>
    res.type('text/plain').send(await metrics.texto()),
  );
  expressApp.get('/health', (_req: Request, res: Response) => res.json(metrics.salud()));
  SwaggerModule.setup('docs', app, crearDocumentoOpenApi(app));
  app.enableShutdownHooks();
  await app.listen(puerto);
  Logger.create(NOMBRE_SERVICIOS.STORES).info({ msg: 'stores-service listo', puerto });
}

bootstrap().catch((err: Error) => {
  console.error(JSON.stringify({ nivel: 'error', msg: 'Fallo el arranque', err: err.message }));
  process.exit(1);
});
