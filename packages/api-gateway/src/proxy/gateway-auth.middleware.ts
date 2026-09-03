import { Request, Response, NextFunction } from 'express';
import {
  verificarToken,
  TokenPayload,
  ROLES,
  ROLES_LOGISTICA,
  Rol,
  Logger,
  TENANT_HEADER,
  PERSONAL_HEADER,
} from '@core/shared';

export interface PoliticaRuta {
  patron: RegExp;
  metodos: string[];
  /** null = publica; lista = roles permitidos. */
  roles: Rol[] | null;
}

/**
 * Enrutado (Tabla 21) + politica de acceso por ruta/rol en el borde.
 * Las rutas publicas no exigen token; las protegidas exigen JWT valido y rol.
 */
export const POLITICAS_RUTAS: PoliticaRuta[] = [
    // publico: identidad y catalogo de lectura
  { patron: /^\/api\/v1\/auth\/registro$/, metodos: ['POST'], roles: null },
    { patron: /^\/api\/v1\/auth\/login$/, metodos: ['POST'], roles: null },
    { patron: /^\/api\/v1\/auth\/refresh$/, metodos: ['POST'], roles: null },
    { patron: /^\/api\/v1\/auth\/restablecer-contrasena$/, metodos: ['POST'], roles: null },
    { patron: /^\/api\/v1\/catalog\/productos$/, metodos: ['GET'], roles: null },
    { patron: /^\/api\/v1\/catalog\/productos\/[^/]+$/, metodos: ['GET'], roles: null },
    { patron: /^\/api\/v1\/tiendas\/[^/]+$/, metodos: ['GET'], roles: null },
    // usuarios: gestion administrativa (sustituye a Firebase gestion-usuarios)
    { patron: /^\/api\/v1\/usuarios(\/|$)/, metodos: ['*'], roles: [ROLES.ADMIN] },
    // creacion administrativa de usuarios (roles de tienda y logistica)
    { patron: /^\/api\/v1\/auth\/crear-usuario$/, metodos: ['POST'], roles: [ROLES.ADMIN] },
    // autenticado
    { patron: /^\/api\/v1\/auth\/me$/, metodos: ['GET'], roles: null }, // exige token (se valida abajo)
    // catálogo: escritura admin
    { patron: /^\/api\/v1\/catalog\/productos/, metodos: ['POST', 'PATCH', 'PUT', 'DELETE'], roles: [ROLES.ADMIN] },
    // vendedores
    {
      patron: /^\/api\/v1\/vendedores\/(me\/ventas|me\/liquidaciones)$/,
      metodos: ['GET'],
      roles: [ROLES.VENDEDOR],
    },
    {
      patron: /^\/api\/v1\/vendedores\//,
      metodos: ['GET', 'POST', 'PATCH', 'PUT', 'DELETE'],
      roles: [ROLES.VENDEDOR],
    },
    // pedidos
    { patron: /^\/api\/v1\/orders$/, metodos: ['POST'], roles: [ROLES.COMPRADOR] },
    { patron: /^\/api\/v1\/orders$/, metodos: ['GET'], roles: [ROLES.COMPRADOR] },
    {
      patron: /^\/api\/v1\/orders\/[^/]+\/estado$/,
      metodos: ['PATCH', 'POST'],
      roles: [ROLES.ADMIN, ROLES.LOGISTICA],
    },
    {
      patron: /^\/api\/v1\/orders\//,
      metodos: ['GET'],
      roles: [ROLES.COMPRADOR, ROLES.VENDEDOR, ROLES.ADMIN],
    },
    // carrito (RN-05): solo el comprador autenticado
    {
      patron: /^\/api\/v1\/carrito/,
      metodos: ['GET', 'POST', 'PATCH', 'DELETE'],
      roles: [ROLES.COMPRADOR],
    },
// admin
  {
    patron: /^\/api\/v1\/admin\/inventario/,
    metodos: ['GET', 'POST', 'PATCH'],
    roles: [ROLES.ADMIN],
  },
  {
    patron: /^\/api\/v1\/admin\//,
    metodos: ['*'],
    roles: [ROLES.ADMIN],
  },
  // finanzas y regimen fiscal (contabilidad, comprobantes, proyecciones)
  {
    patron: /^\/api\/v1\/finanzas\//,
    metodos: ['*'],
    roles: [ROLES.ADMIN],
  },
  // field-service: logistica de campo (app-test) — roles de logistica
  {
    patron: /^\/api\/v1\/field\//,
    metodos: ['*'],
    roles: [...ROLES_LOGISTICA],
  },
  // market-intelligence: admin ve todo, vendedor ve lo suyo, endpoints internos protegidos
  {
    patron: /^\/api\/v1\/inteligencia\/me\//,
    metodos: ['GET'],
    roles: [ROLES.VENDEDOR, ROLES.ADMIN],
  },
  {
    patron: /^\/api\/v1\/inteligencia\//,
    metodos: ['GET'],
    roles: [ROLES.ADMIN],
  },
  {
    patron: /^\/api\/v1\/inteligencia\/ventas\/registrar$/,
    metodos: ['POST'],
    roles: [ROLES.VENDEDOR, ROLES.COMPRADOR, ...ROLES_LOGISTICA],
  },
];

/**
 * Middleware de autenticacion: verifica el JWT (HS256, Supabase-compatible) y
 * propaga el contexto a los servicios con cabeceras X-User-*.
 * Las rutas publicas pasan sin token; el resto exige sesion valida.
 */
export class GatewayAuthMiddleware {
  private readonly logger = Logger.create('gateway.auth');

  constructor(private readonly secreto = process.env.JWT_SECRET ?? 'dev_secret') {}

  aplicar(req: Request, res: Response, next: NextFunction, politica: PoliticaRuta | null): void {
    const autorizacion = req.headers['authorization'] as string | undefined;
    const tieneToken = !!autorizacion?.startsWith('Bearer ');
    const necesitaToken = !politica || politica.roles !== null;

    if (!tieneToken) {
      if (necesitaToken) {
        res.status(401).json({ codigo: 'NO_AUTORIZADO', mensaje: 'Se requiere un token JWT valido.' });
        return;
      }
      return next();
    }

    try {
      const token = autorizacion!.slice(7);
      const payload = verificarToken<TokenPayload>(token, this.secreto);
      if (payload.tipo !== 'access') {
        res.status(401).json({ codigo: 'NO_AUTORIZADO', mensaje: 'Token de refresco no es valido para acceso.' });
        return;
      }
      req.headers['x-user-id'] = payload.sub;
      req.headers['x-user-email'] = payload.email;
      req.headers['x-user-rol'] = payload.rol;
      if (payload.nombre) req.headers['x-user-nombre'] = payload.nombre;
      if (payload.tenant_id) req.headers[TENANT_HEADER] = payload.tenant_id;
      if (payload.personal_id) req.headers[PERSONAL_HEADER] = payload.personal_id;

      if (politica?.roles && !politica.roles.includes(payload.rol)) {
        this.logger.warn({ msg: 'acceso denegado por rol', ruta: req.originalUrl, rol: payload.rol });
        res.status(403).json({
          codigo: 'ACCESO_DENEGADO',
          mensaje: `Rol "${payload.rol}" no tiene acceso a este recurso.`,
        });
        return;
      }
      return next();
    } catch {
      res.status(401).json({ codigo: 'NO_AUTORIZADO', mensaje: 'Token invalido o expirado.' });
    }
  }
}