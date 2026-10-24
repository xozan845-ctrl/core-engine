import type { Request, Response, NextFunction } from 'express';

/**
 * Cabeceras de hardening del borde (barrido A05). Se aplican a todas las
 * respuestas del gateway, incluidas las rutas registradas fuera de Nest.
 *
 * COOP/COEP/CORP y Permissions-Policy cierran vectores de aislamiento del
 * navegador y reducen el fingerprint; `X-Permitted-Cross-Domain-Policies`
 * desactiva las politicas cross-domain de Flash/PDF.
 */
export function securityHeaders() {
  return (_req: Request, res: Response, next: NextFunction): void => {
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('X-Frame-Options', 'DENY');
    res.setHeader('Referrer-Policy', 'no-referrer');
    res.setHeader("Content-Security-Policy", "default-src 'none'; frame-ancestors 'none'");
    res.setHeader('Strict-Transport-Security', 'max-age=31536000; includeSubDomains');
    res.setHeader(
      'Permissions-Policy',
      'camera=(), microphone=(), geolocation=(), browsing-topics=()',
    );
    res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
    res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
    res.setHeader('Cross-Origin-Resource-Policy', 'same-site');
    res.setHeader('X-Permitted-Cross-Domain-Policies', 'none');
    next();
  };
}
