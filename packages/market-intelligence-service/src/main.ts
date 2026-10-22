import { NestFactory } from '@nestjs/core';
import { SwaggerModule } from '@nestjs/swagger';
import type { Request, Response } from 'express';
import { AppModule } from './app.module';
import { crearDocumentoOpenApi } from './swagger';
import {
  MetricsService,
  PUERTOS,
  NOMBRE_SERVICIOS,
  DtoValidationPipe,
  DomainErrorFilter,
  Logger,
  RolesGuard,
} from '@core/shared';

async function bootstrap(): Promise<void> {
  const app = await NestFactory.create(AppModule, { logger: false });

  app.useGlobalPipes(new DtoValidationPipe());
  app.useGlobalFilters(new DomainErrorFilter());
  app.useGlobalGuards(new RolesGuard());

  const puerto = Number(process.env.PORT ?? PUERTOS.INTELLIGENCE);
  const expressApp = app.getHttpAdapter().getInstance();
  const metrics = app.get(MetricsService);
  expressApp.get('/metrics', async (_req: Request, res: Response) =>
    res.type('text/plain').send(await metrics.texto()),
  );
  expressApp.get('/health', (_req: Request, res: Response) => res.json(metrics.salud()));

  SwaggerModule.setup('docs', app, crearDocumentoOpenApi(app));
  app.enableShutdownHooks();
  await app.listen(puerto);
  Logger.create(NOMBRE_SERVICIOS.INTELLIGENCE).info({
    msg: 'market-intelligence-service listo',
    puerto,
  });
}

bootstrap().catch((err: Error) => {
  console.error(JSON.stringify({ nivel: 'error', msg: 'Fallo el arranque', err: err.message }));
  process.exit(1);
});
