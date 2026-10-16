import type { Request, Response, NextFunction } from 'express';
import { timingSafeEqual } from 'crypto';

/**
 * Endurecimiento del borde (R-DS-3, R-GW-1): gate opcional de Basic Auth para
 * entornos que no deben ser publicos (staging) y cierre de la superficie de
 * diagnostico (`/docs` con todo el contrato y `/metrics` con las metricas
 * internas).
 *
 * `/health` queda SIEMPRE abierto: es el liveness que consultan Docker/Dockploy
 * y no revela nada del contrato. `/ready` y el resto de rutas pasan por el gate
 * cuando se configura.
 */
export interface OpcionesEdge {
  /** `usuario:clave` para exigir Basic Auth en el borde. Vacio = sin gate. */
  basicAuth?: string;
  /** `true` sirve `/docs` y `/metrics` sin restriccion (desarrollo local). */
  diagnosticosPublicos: boolean;
  /** Clave interna que permite diagnosticos aunque no sean publicos. */
  claveInterna?: string;
}

// `/docs` (UI) y `/docs-json` / `/docs-yaml` (el contrato completo) + `/metrics`.
const RUTAS_DIAGNOSTICO = [/^\/docs(-|\/|$)/, /^\/metrics$/];

/** Comparacion en tiempo constante para no filtrar la credencial por prefijo. */
function iguales(a: string, b: string): boolean {
  const bufferA = Buffer.from(a);
  const bufferB = Buffer.from(b);
  if (bufferA.length !== bufferB.length) return false;
  return timingSafeEqual(bufferA, bufferB);
}

/** Lee la configuracion del borde desde el entorno (R-CD-7). */
export function leerEdge(env: NodeJS.ProcessEnv = process.env): OpcionesEdge {
  return {
    basicAuth: env.EDGE_BASIC_AUTH?.trim() || undefined,
    diagnosticosPublicos: (env.DIAGNOSTICOS_PUBLICOS ?? 'false').toLowerCase() === 'true',
    claveInterna: env.INTERNAL_API_KEY || undefined,
  };
}

/** Middleware de Express con el gate del borde. */
export function edgeAuth(opciones: OpcionesEdge = leerEdge()) {
  return (req: Request, res: Response, next: NextFunction): void => {
    // Liveness: nunca detras del gate (healthcheck de contenedor/Dockploy).
    if (req.path === '/health') {
      next();
      return;
    }

    if (opciones.basicAuth) {
      const cabecera = req.headers.authorization ?? '';
      const [esquema, credencial] = cabecera.split(' ');
      const plano = credencial ? Buffer.from(credencial, 'base64').toString('utf-8') : '';
      if (esquema !== 'Basic' || !credencial || !iguales(plano, opciones.basicAuth)) {
        res.setHeader('WWW-Authenticate', 'Basic realm="Core Engine"');
        res.status(401).json({
          codigo: 'NO_AUTORIZADO',
          mensaje: 'Se requiere autenticacion de borde para este entorno.',
        });
        return;
      }
    }

    const esDiagnostico = RUTAS_DIAGNOSTICO.some((patron) => patron.test(req.path));
    if (esDiagnostico && !opciones.diagnosticosPublicos) {
      const clave = req.headers['x-internal-key'];
      if (!opciones.claveInterna || clave !== opciones.claveInterna) {
        // 404 (no 401/403): no se anuncia que la ruta existe (menos fingerprinting).
        res.status(404).json({
          codigo: 'NO_ENCONTRADO',
          mensaje: `No existe ruta ${req.method} ${req.path}.`,
        });
        return;
      }
    }

    next();
  };
}
