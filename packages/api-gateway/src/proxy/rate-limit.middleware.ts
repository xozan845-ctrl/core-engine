import { Injectable, NestMiddleware } from '@nestjs/common';
import { Request, Response, NextFunction } from 'express';

interface VentanaLimite {
  inicio: number;
  conteo: number;
}

/**
 * Limitacion de tasa en el borde (doc 5.4: gateway como limite).
 * Ventana fija por IP en memoria: global 300 peticiones/min por IP y una
 * ventana estricta para /auth/login (10/min, anti fuerza bruta).
 * Respuesta 429 estructurada (doc 5.7): {codigo, mensaje, detalles}.
 */
@Injectable()
export class RateLimitMiddleware implements NestMiddleware {
  private readonly ventanas = new Map<string, VentanaLimite>();
  private readonly globales = { ttl_ms: 60_000, limite: 300 };
  private readonly login = { ttl_ms: 60_000, limite: 10 };

  constructor() {
    // limpieza periodica para no acumular entradas en memoria; unref evita
    // que el timer mantenga vivo el proceso en tests
    const limpieza = setInterval(() => {
      const ahora = Date.now();
      for (const [clave, v] of this.ventanas) {
        if (ahora - v.inicio > this.globales.ttl_ms) this.ventanas.delete(clave);
      }
    }, 60_000);
    limpieza.unref?.();
  }

  use(req: Request, res: Response, next: NextFunction): void {
    const ip = req.ip ?? req.socket.remoteAddress ?? 'desconocida';
    const esLogin = req.method === 'POST' && /\/api\/v1\/auth\/login$/.test(req.path);
    const config = esLogin ? this.login : this.globales;
    const clave = `${esLogin ? 'login' : 'global'}:${ip}`;

    const ahora = Date.now();
    const anterior = this.ventanas.get(clave);
    if (!anterior || ahora - anterior.inicio >= config.ttl_ms) {
      this.ventanas.set(clave, { inicio: ahora, conteo: 1 });
    } else {
      anterior.conteo += 1;
      if (anterior.conteo > config.limite) {
        const restante = Math.max(0, Math.ceil((anterior.inicio + config.ttl_ms - ahora) / 1000));
        res.setHeader('Retry-After', String(restante));
        res.setHeader('x-ratelimit-limit', String(config.limite));
        res.setHeader('x-ratelimit-remaining', '0');
        res.status(429).json({
          codigo: 'DEMASIADAS_PETICIONES',
          mensaje: 'Se excedio el limite de peticiones; intente de nuevo en unos segundos.',
          detalles: { limite: config.limite, ventana_seg: Math.floor(config.ttl_ms / 1000), reintente_en_seg: restante },
        });
        return;
      }
    }
    res.setHeader('x-ratelimit-limit', String(config.limite));
    res.setHeader('x-ratelimit-remaining', String(Math.max(0, config.limite - (this.ventanas.get(clave)?.conteo ?? 0))));
    next();
  }
}