import type { Request, Response, NextFunction } from 'express';
import { timingSafeEqual } from 'crypto';

/**
 * Cierre de la superficie de diagnóstico sin gate de borde (ADR-0005, R-GW-8).
 *
 * `/health` es siempre público (liveness de Docker/Dockploy). `/ready`, `/docs*`
 * y `/metrics` exigen `x-internal-key` salvo que el entorno los declare
 * públicos para desarrollo. La privacidad de staging ya no se resuelve aquí:
 * vive en el borde externo (Cloudflare Access/VPN/allowlist).
 */
export interface OpcionesDiagnostico {
  /** `true` sirve `/docs` y `/metrics` sin restricción (desarrollo local). */
  diagnosticosPublicos: boolean;
  /** `true` deja `/ready` accesible sin clave interna (solo desarrollo). */
  readyPublico: boolean;
  /** Clave interna que permite diagnóstico y `/ready`. */
  claveInterna?: string;
}

// `/docs` (UI) y `/docs-json` / `/docs-yaml` (el contrato completo) + `/metrics`.
const RUTAS_DIAGNOSTICO = [/^\/docs(-|\/|$)/, /^\/metrics$/];

function valorCabecera(valor: string | string[] | undefined): string | undefined {
  return Array.isArray(valor) ? valor[0] : valor;
}

/** Comparación en tiempo constante para no filtrar la credencial por prefijo. */
function iguales(a: string, b: string): boolean {
  const bufferA = Buffer.from(a);
  const bufferB = Buffer.from(b);
  if (bufferA.length !== bufferB.length) return false;
  return timingSafeEqual(bufferA, bufferB);
}

/** Lee la configuración desde el entorno (R-CD-7). */
export function leerDiagnostico(env: NodeJS.ProcessEnv = process.env): OpcionesDiagnostico {
  return {
    diagnosticosPublicos: (env.DIAGNOSTICOS_PUBLICOS ?? 'false').toLowerCase() === 'true',
    readyPublico: (env.READY_PUBLIC ?? 'false').toLowerCase() === 'true',
    claveInterna: env.INTERNAL_API_KEY || undefined,
  };
}

function oculto(req: Request, res: Response): void {
  // 404 (no 401/403): no se anuncia que la ruta existe (menos fingerprinting).
  res.status(404).json({
    codigo: 'NO_ENCONTRADO',
    mensaje: `No existe ruta ${req.method} ${req.path}.`,
  });
}

/** Middleware de Express que cierra la superficie de diagnóstico. */
export function diagnostico(opciones: OpcionesDiagnostico = leerDiagnostico()) {
  return (req: Request, res: Response, next: NextFunction): void => {
    const clave = valorCabecera(req.headers['x-internal-key']);
    const claveValida =
      !!clave && !!opciones.claveInterna && iguales(clave, opciones.claveInterna);

    if (req.path === '/ready') {
      if (claveValida || opciones.readyPublico) {
        next();
        return;
      }
      oculto(req, res);
      return;
    }

    const esDiagnostico = RUTAS_DIAGNOSTICO.some((patron) => patron.test(req.path));
    if (esDiagnostico && !opciones.diagnosticosPublicos && !claveValida) {
      oculto(req, res);
      return;
    }

    next();
  };
}
