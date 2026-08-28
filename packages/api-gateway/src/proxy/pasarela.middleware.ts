import { Injectable, NestMiddleware } from '@nestjs/common';
import { Request, Response, NextFunction } from 'express';
import { randomUUID } from 'crypto';
import {
  MetricsService,
  ejecutarConContexto,
  ContextoCorrelacion,
  NOMBRE_SERVICIOS,
} from '@core/shared';
import { ProxyService } from './proxy.service';

/**
 * Middleware comodin de la pasarela: enruta /api/v1/* hacia los servicios.
 * Genera el x-request-id de borde (doc 5.3: correlacion distribuida) y registra
 * las metricas http_* del gateway en cada peticion.
 * Se registra para todas las rutas y filtra por prefijo, evitando
 * dependencias del patron de comodin del router de Express.
 */
@Injectable()
export class PasarelaMiddleware implements NestMiddleware {
  constructor(
    private readonly proxy: ProxyService,
    private readonly metrics: MetricsService,
  ) {}

  use(req: Request, res: Response, next: NextFunction): void {
    const inicio = Date.now();
    const request_id = (req.headers['x-request-id'] as string) ?? randomUUID();
    req.headers['x-request-id'] = request_id;
    res.setHeader('x-request-id', request_id);
    res.on('finish', () => {
      this.metrics.registrarPeticion(
        req.method ?? 'GET',
        req.originalUrl ?? req.path ?? '/',
        res.statusCode ?? 500,
        Date.now() - inicio,
      );
    });

    if (!req.path.startsWith('/api/v1/')) return next();

    const contexto: ContextoCorrelacion = {
      request_id,
      servicio: NOMBRE_SERVICIOS.GATEWAY,
    };
    ejecutarConContexto(contexto, () => {
      this.proxy.reenviar(req, res).catch((err) => {
        if (res.headersSent) {
          res.end();
          return;
        }
        res.status(502).json({
          codigo: 'DESTINO_NO_DISPONIBLE',
          mensaje: err instanceof Error ? err.message : 'Fallo interno del gateway.',
        });
      });
    });
  }
}