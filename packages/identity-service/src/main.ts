import 'reflect-metadata';
import { NestFactory } from '@nestjs/core';
import type { Request, Response } from 'express';
import { AppModule } from './app.module';
import {
  MetricsService,
  TrazabilidadInterceptor,
  DtoValidationPipe,
  DomainErrorFilter,
  NOMBRE_SERVICIOS,
  PUERTOS,
  Logger,
} from '@core/shared';

async function bootstrap(): Promise<void> {
  const app = await NestFactory.create(AppModule, { logger: false });
  const logger = Logger.create(NOMBRE_SERVICIOS.IDENTITY);
  app.useGlobalPipes(new DtoValidationPipe());
  app.useGlobalInterceptors(
    new TrazabilidadInterceptor(app.get(MetricsService), NOMBRE_SERVICIOS.IDENTITY),
  );
  app.useGlobalFilters(new DomainErrorFilter());
  const puerto = Number(process.env.PORT ?? PUERTOS.IDENTITY);
  const expressApp = app.getHttpAdapter().getInstance();

  const metrics = app.get(MetricsService);
  expressApp.get('/metrics', async (_req: Request, res: Response) =>
    res.type('text/plain').send(await metrics.texto()),
  );
  expressApp.get('/health', (_req: Request, res: Response) => res.json(metrics.salud()));

  await app.listen(puerto);
  logger.info({ msg: 'identity-service listo', puerto });
}

bootstrap().catch((err: Error) => {
  console.error(JSON.stringify({ nivel: 'error', msg: 'Fallo el arranque', err: err.message }));
  process.exit(1);
});