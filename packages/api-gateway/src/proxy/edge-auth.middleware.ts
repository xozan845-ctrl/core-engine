import type { Request, Response, NextFunction } from 'express';
import { timingSafeEqual } from 'crypto';
import { verificarToken, type TokenPayload } from '@core/shared';

/**
 * Endurecimiento del borde (R-DS-3, R-GW-1): gate opcional de Basic Auth para
 * entornos que no deben ser publicos (staging) y cierre de la superficie de
 * diagnostico (`/docs` con todo el contrato y `/metrics` con las metricas
 * internas).
 *
 * `/health` queda SIEMPRE abierto: es el liveness que consultan Docker/Dockploy
 * y no revela nada del contrato. `/ready` exige `x-internal-key` salvo que
 * `READY_PUBLIC=true` (solo desarrollo); asi el smoke/healthcheck no necesita
 * exponer la topologia al publico.
 */
export interface OpcionesEdge {
  /** `usuario:clave` para exigir Basic Auth en el borde. Vacio = sin gate. */
  basicAuth?: string;
  /** `true` sirve `/docs` y `/metrics` sin restriccion (desarrollo local). */
  diagnosticosPublicos: boolean;
  /** Clave interna que permite diagnostico y `/ready` aunque no sean publicos. */
  claveInterna?: string;
  /** `true` deja `/ready` accesible sin clave interna (solo desarrollo). */
  readyPublico?: boolean;
  /** Secreto HS256 para validar el Bearer que sustituye al gate de Basic. */
  jwtSecret?: string;
}

// `/docs` (UI) y `/docs-json` / `/docs-yaml` (el contrato completo) + `/metrics`.
const RUTAS_DIAGNOSTICO = [/^\/docs(-|\/|$)/, /^\/metrics$/];

// Imágenes de producto: públicas por diseño (ADR-28), incluso en staging.
const RUTAS_ESTATICAS = [/^\/api\/v1\/storage\/public\/objetos\/[^/]+$/];

/** Comparacion en tiempo constante para no filtrar la credencial por prefijo. */
function iguales(a: string, b: string): boolean {
  const bufferA = Buffer.from(a);
  const bufferB = Buffer.from(b);
  if (bufferA.length !== bufferB.length) return false;
  return timingSafeEqual(bufferA, bufferB);
}

function valorCabecera(valor: string | string[] | undefined): string | undefined {
  return Array.isArray(valor) ? valor[0] : valor;
}

/** Lee la configuracion del borde desde el entorno (R-CD-7). */
export function leerEdge(env: NodeJS.ProcessEnv = process.env): OpcionesEdge {
  return {
    basicAuth: env.EDGE_BASIC_AUTH?.trim() || undefined,
    diagnosticosPublicos: (env.DIAGNOSTICOS_PUBLICOS ?? 'false').toLowerCase() === 'true',
    claveInterna: env.INTERNAL_API_KEY || undefined,
    readyPublico: (env.READY_PUBLIC ?? 'false').toLowerCase() === 'true',
    jwtSecret: env.JWT_SECRET || undefined,
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

    if (RUTAS_ESTATICAS.some((patron) => patron.test(req.path))) {
      next();
      return;
    }

    // `/ready` es operativo: la clave interna lo abre sin exigir Basic (asi el
    // smoke de despliegue no necesita credenciales de usuario). Sin clave, solo
    // es publico si READY_PUBLIC=true; en caso contrario se oculta con 404.
    if (req.path === '/ready') {
      const clave = valorCabecera(req.headers['x-internal-key']);
      if (clave && opciones.claveInterna && iguales(clave, opciones.claveInterna)) {
        next();
        return;
      }
      if (!opciones.readyPublico) {
        res.status(404).json({
          codigo: 'NO_ENCONTRADO',
          mensaje: `No existe ruta ${req.method} ${req.path}.`,
        });
        return;
      }
    }

    if (opciones.basicAuth) {
      // Basic y Bearer comparten la cabecera `Authorization`. Un Bearer solo
      // sustituye al gate si el JWT es valido: de lo contrario cualquiera
      // podria saltarse el Basic de staging con "Authorization: Bearer x".
      const autorizacion = req.headers.authorization ?? '';
      const traeBearer = /^Bearer\s+\S+/i.test(autorizacion);
      if (traeBearer) {
        const token = autorizacion.replace(/^Bearer\s+/i, '').trim();
        try {
          const payload = verificarToken<TokenPayload>(token, opciones.jwtSecret ?? '');
          if (payload.tipo !== 'access') throw new Error('tipo de token invalido');
        } catch {
          res.setHeader('WWW-Authenticate', 'Bearer');
          res.status(401).json({
            codigo: 'NO_AUTORIZADO',
            mensaje: 'Token invalido o expirado.',
          });
          return;
        }
      } else {
        const [esquema, credencial] = autorizacion.split(' ');
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
    }

    const esDiagnostico = RUTAS_DIAGNOSTICO.some((patron) => patron.test(req.path));
    if (esDiagnostico && !opciones.diagnosticosPublicos) {
      const clave = valorCabecera(req.headers['x-internal-key']);
      if (!opciones.claveInterna || !clave || !iguales(clave, opciones.claveInterna)) {
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
