import { Injectable, NestMiddleware } from '@nestjs/common';
import { Request, Response, NextFunction } from 'express';

interface VentanaLimite {
  inicio: number;
  conteo: number;
}

export interface OpcionesLimite {
  /** Maximo de peticiones por ventana. */
  limite: number;
  /** Duracion de la ventana en milisegundos. */
  ttlMs: number;
  /** Nombre del cubo; permite separar login/ready del limite global. */
  clave?: string;
}

/**
 * Limitacion de tasa en el borde (doc 5.4: gateway como limite).
 * Ventana fija por IP en memoria: global 300 peticiones/min por IP y una
 * ventana estricta para /auth/login (10/min, anti fuerza bruta).
 * Respuesta 429 estructurada (doc 5.7): {codigo, mensaje, detalles}.
 *
 * `consumir` se expone para rutas registradas fuera del middleware de Nest
 * (`/ready`), que de otro modo quedarian sin limite.
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

  /**
   * Consume una peticion del cubo indicado. Devuelve `false` y responde 429 si
   * se excede el limite. Las cabeceras `x-ratelimit-*` se emiten siempre.
   */
  consumir(req: Request, res: Response, opciones: OpcionesLimite): boolean {
    const ip = req.ip ?? req.socket.remoteAddress ?? 'desconocida';
    const clave = `${opciones.clave ?? 'global'}:${ip}`;

    const ahora = Date.now();
    const anterior = this.ventanas.get(clave);
    let conteo: number;
    if (!anterior || ahora - anterior.inicio >= opciones.ttlMs) {
      conteo = 1;
      this.ventanas.set(clave, { inicio: ahora, conteo });
    } else {
      anterior.conteo += 1;
      conteo = anterior.conteo;
    }

    res.setHeader('x-ratelimit-limit', String(opciones.limite));
    if (conteo > opciones.limite) {
      const ventana = this.ventanas.get(clave);
      const restante = Math.max(
        0,
        Math.ceil(((ventana?.inicio ?? ahora) + opciones.ttlMs - ahora) / 1000),
      );
      res.setHeader('Retry-After', String(restante));
      res.setHeader('x-ratelimit-remaining', '0');
      res.status(429).json({
        codigo: 'DEMASIADAS_PETICIONES',
        mensaje: 'Se excedio el limite de peticiones; intente de nuevo en unos segundos.',
        detalles: {
          limite: opciones.limite,
          ventana_seg: Math.floor(opciones.ttlMs / 1000),
          reintente_en_seg: restante,
        },
      });
      return false;
    }

    res.setHeader('x-ratelimit-remaining', String(Math.max(0, opciones.limite - conteo)));
    return true;
  }

  use(req: Request, res: Response, next: NextFunction): void {
    // Las imágenes públicas son estáticas y cacheables: no deben consumir el
    // cubo de la API (una página de catálogo puede cargar decenas).
    if (req.method === 'GET' && /^\/api\/v1\/storage\/public\/objetos\/[^/]+$/.test(req.path)) {
      next();
      return;
    }
    const esLogin = req.method === 'POST' && /\/api\/v1\/auth\/login$/.test(req.path);
    const opciones: OpcionesLimite = esLogin
      ? { limite: this.login.limite, ttlMs: this.login.ttl_ms, clave: 'login' }
      : { limite: this.globales.limite, ttlMs: this.globales.ttl_ms };

    if (this.consumir(req, res, opciones)) next();
  }
}
