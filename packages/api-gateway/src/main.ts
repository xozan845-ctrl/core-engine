import { NestFactory } from '@nestjs/core';
import { DocumentBuilder, SwaggerModule } from '@nestjs/swagger';
import type { Request, Response } from 'express';
import { AppModule } from './app.module';
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

  // salud agregada: liveness del gateway + readiness de cada microservicio
  expressApp.get('/health', async (_req: Request, res: Response) => {
const servicios: [string, number][] = [
      ['identity', 3001],
      ['catalog', 3002],
      ['stores', 3003],
      ['orders', 3004],
      ['logistics', 3005],
      ['commissions', 3006],
      ['finance', 3007],
    ];
    const estado: Record<string, string> = {};
    await Promise.all(
      servicios.map(async ([s, puertoSvc]) => {
        const url = `http://${s}-service:${puertoSvc}/health`;
        try {
          const r = await fetch(url, { signal: AbortSignal.timeout(1500) });
          estado[s] = r.ok ? 'ok' : `error:${r.status}`;
        } catch {
          estado[s] = 'caido';
        }
      }),
    );
    res.json({ api_gateway: 'ok', servicios: estado });
  });

  // Swagger (DoD: la API esta documentada; versionado AD-05 /api/v1)
  const config = new DocumentBuilder()
    .setTitle('BodegaHub · Core Engine API')
    .setDescription('API Gateway de la plataforma de comercio electronico distribuido')
    .setVersion('v1')
    .addBearerAuth()
    .build();
  const documento = SwaggerModule.createDocument(app, config);
  SwaggerModule.setup('docs', app, documento);

  await app.listen(puerto);
  logger.info({ msg: 'api-gateway listo', puerto, docs: '/docs' });
}

bootstrap().catch((err: Error) => {
  console.error(JSON.stringify({ nivel: 'error', msg: 'Fallo el arranque', err: err.message }));
  process.exit(1);
});