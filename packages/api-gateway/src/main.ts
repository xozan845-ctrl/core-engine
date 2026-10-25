import { NestFactory } from '@nestjs/core';
import { SwaggerModule } from '@nestjs/swagger';
import type { NextFunction, Request, Response } from 'express';
import { AppModule } from './app.module';
import { OPENAPI_AGREGADO } from './openapi.agregado';
import {
  evaluarReadiness,
  codigoReadiness,
  type EstadoReadiness,
} from './health/readiness.utils';
import { edgeAuth, leerEdge } from './proxy/edge-auth.middleware';
import { crearProxyPublico } from './proxy/public-proxy.middleware';
import { RateLimitMiddleware } from './proxy/rate-limit.middleware';
import { securityHeaders } from './proxy/security-headers.middleware';
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
  // nunca reflejar origenes arbitrarios (barrido A05). Con gate de borde activo
  // (staging) se admiten credenciales: el navegador envia el Basic Auth.
  const edge = leerEdge();
  // Staging exige el gate de borde (M-01): fallar el arranque es preferible a
  // desplegar un entorno interno sin autenticacion.
  if ((process.env.REQUIRE_EDGE_AUTH ?? 'false').toLowerCase() === 'true' && !edge.basicAuth) {
    throw new Error('EDGE_BASIC_AUTH es obligatoria cuando REQUIRE_EDGE_AUTH=true.');
  }
  const origenes = (process.env.CORS_ORIGINS ?? '').split(',').map((o) => o.trim()).filter(Boolean);
  app.enableCors({
    origin: origenes,
    credentials: Boolean(edge.basicAuth),
    methods: ['GET', 'POST', 'PATCH', 'PUT', 'DELETE', 'OPTIONS'],
    allowedHeaders: ['Content-Type', 'Authorization', 'x-request-id'],
    exposedHeaders: ['x-request-id'],
  });
  app.useGlobalFilters(new DomainErrorFilter());

  const expressApp = app.getHttpAdapter().getInstance();

  // Traefik/Caddy es el unico proxy delante del gateway. Sin `trust proxy`,
  // `req.ip` seria la IP del proxy y TODOS los clientes compartirian el cubo de
  // rate limit (300/min y 10/min de login). `TRUST_PROXY=false` desactiva la
  // confianza cuando se accede directamente (desarrollo).
  const trustProxy = process.env.TRUST_PROXY ?? '1';
  if (trustProxy !== 'false' && trustProxy !== '0') {
    expressApp.set('trust proxy', /^\d+$/.test(trustProxy) ? Number(trustProxy) : trustProxy);
  }

  // Hardening de cabeceras y fingerprint (barrido A05): sin X-Powered-By y
  // cabeceras de seguridad base + aislamiento del navegador en el borde. Van
  // antes del gate para que tambien las respuestas 401/404 las lleven.
  expressApp.disable('x-powered-by');
  expressApp.use(securityHeaders());
  // Proxy server-side para el frontend público (ADR-29). Va ANTES del gate:
  // el handler añade el Basic internamente y nunca lo expone al navegador.
  const proxyPublico = crearProxyPublico({
    basicAuth: edge.basicAuth,
    origenes,
    puerto,
  });
  expressApp.use('/api-proxy', (req: Request, res: Response, next: NextFunction) => {
    proxyPublico(req, res).catch(next);
  });
  // Gate del borde ANTES de cualquier ruta (incluidas /metrics y /docs).
  expressApp.use(edgeAuth(edge));
  const metrics = app.get(MetricsService);
  expressApp.get('/metrics', async (_req: Request, res: Response) =>
    res.type('text/plain').send(await metrics.texto()),
  );

  // Liveness: el gateway responde mientras el proceso esta vivo (healthcheck Docker).
  expressApp.get('/health', (_req: Request, res: Response) =>
    res.json({ api_gateway: 'ok', ...metrics.salud() }),
  );

  // Readiness (R-CD-12): 503 si algun microservicio de produccion no responde.
  // Esta ruta se registra directamente en Express y no pasa por el middleware
  // de Nest: se limita aqui y se cachea para que una rafaga no amplifique a 8
  // peticiones internas por cada peticion externa (H-02).
  const limitador = app.get(RateLimitMiddleware);
  const readyCacheMs = Math.max(0, Number(process.env.READY_CACHE_MS ?? 5000));
  let readyCache: { estado: EstadoReadiness; expira: number } | undefined;

  expressApp.get('/ready', async (req: Request, res: Response) => {
    if (!limitador.consumir(req, res, { limite: 10, ttlMs: 60_000, clave: 'ready' })) return;

    const ahora = Date.now();
    if (!readyCache || readyCache.expira <= ahora) {
      readyCache = { estado: await evaluarReadiness(), expira: ahora + readyCacheMs };
    }
    res.status(codigoReadiness(readyCache.estado)).json(readyCache.estado);
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