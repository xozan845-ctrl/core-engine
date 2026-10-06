import { NestFactory } from '@nestjs/core';
import { SwaggerModule } from '@nestjs/swagger';
import type { Request, Response } from 'express';
import { AppModule } from './app.module';
import { OPENAPI_AGREGADO } from './openapi.agregado';
import { evaluarReadiness, codigoReadiness } from './health/readiness.utils';
import {
  MetricsService,
  DomainErrorFilter,
  NOMBRE_SERVICIOS,
  PUERTOS,
  Logger,
} from '@core/shared';

async function bootstrap(): Promise<void> {
  const app = await NestFactory.create(AppModule, { logger: ['error'] });
  const logger = Logger.create(NOMBRE_SERVICIOS.GATEWAY);
  const puerto = Number(process.env.PORT ?? PUERTOS.GATEWAY);

  // CORS controlado en el borde (doc 5.4): whitelist explicita del entorno;
  // nunca reflejar origenes arbitrarios (barrido A05)
  const origenes = (process.env.CORS_ORIGINS ?? '').split(',').map((o) => o.trim()).filter(Boolean);
  app.enableCors({
    origin: origenes,
    methods: ['GET', 'POST', 'PATCH', 'PUT', 'DELETE', 'OPTIONS'],
    allowedHeaders: ['Content-Type', 'Authorization', 'x-request-id'],
    exposedHeaders: ['x-request-id'],
  });
  app.useGlobalFilters(new DomainErrorFilter());

  const expressApp = app.getHttpAdapter().getInstance();
  // Hardening de cabeceras y fingerprint (barrido A05): sin X-Powered-By y
  // cabeceras de seguridad base en el borde
  expressApp.disable('x-powered-by');
  expressApp.use((_req: Request, res: Response, next: () => void) => {
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('X-Frame-Options', 'DENY');
    res.setHeader('Referrer-Policy', 'no-referrer');
    res.setHeader("Content-Security-Policy", "default-src 'none'; frame-ancestors 'none'");
    res.setHeader('Strict-Transport-Security', 'max-age=31536000; includeSubDomains');
    next();
  });
  const metrics = app.get(MetricsService);
  expressApp.get('/metrics', async (_req: Request, res: Response) =>
    res.type('text/plain').send(await metrics.texto()),
  );

  // Liveness: el gateway responde mientras el proceso esta vivo (healthcheck Docker).
  expressApp.get('/health', (_req: Request, res: Response) =>
    res.json({ api_gateway: 'ok', ...metrics.salud() }),
  );

  // Readiness (R-CD-12): 503 si algun microservicio de produccion no responde.
  expressApp.get('/ready', async (_req: Request, res: Response) => {
    const estado = await evaluarReadiness();
    res.status(codigoReadiness(estado)).json(estado);
  });

  // Swagger: documento AGREGADO de todos los microservicios (R-DO-6, gate G-8).
  SwaggerModule.setup('docs', app, OPENAPI_AGREGADO);

  app.enableShutdownHooks();
  await app.listen(puerto);
  logger.info({ msg: 'api-gateway listo', puerto, docs: '/docs' });
}

bootstrap().catch((err: Error) => {
  console.error(JSON.stringify({ nivel: 'error', msg: 'Fallo el arranque', err: err.message }));
  process.exit(1);
});