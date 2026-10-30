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
  PARTICIPACION_HEADER,
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
    // Invitaciones de vendedor: el token es la autorización temporal.
    { patron: /^\/api\/v1\/invitaciones\/vendedores\/[^/]+$/, metodos: ['GET'], roles: null },
    { patron: /^\/api\/v1\/invitaciones\/vendedores\/[^/]+\/aceptar$/, metodos: ['POST'], roles: null },
    { patron: /^\/api\/v1\/catalog\/productos$/, metodos: ['GET'], roles: null },
    { patron: /^\/api\/v1\/catalog\/productos\/[^/]+$/, metodos: ['GET'], roles: null },
    // Servicios de empresa: recurso administrativo del catálogo (R-PS-02).
    {
      patron: /^\/api\/v1\/catalog\/servicios(\/|$)/,
      metodos: ['*'],
      roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO],
    },
    // Operaciones avanzadas del catálogo (R-PS-04).
    {
      patron: /^\/api\/v1\/catalog\/(configuracion-inventario|escanear|combos|promociones|inventario)(\/|$)/,
      metodos: ['*'],
      roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO],
    },
    // El slug de tienda forma parte de la URL publica del storefront (ADR-26);
    // el cambio de slug exige ser vendedor/empresario y se valida en el servicio.
    { patron: /^\/api\/v1\/tiendas\/[^/]+\/slug$/, metodos: ['PATCH'], roles: [ROLES.VENDEDOR, ROLES.EMPRESARIO] },
    { patron: /^\/api\/v1\/tiendas\/[^/]+$/, metodos: ['GET'], roles: null },
    // usuarios: gestion administrativa (sustituye a Firebase gestion-usuarios)
    { patron: /^\/api\/v1\/usuarios(\/|$)/, metodos: ['*'], roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN] },
    // creacion administrativa de usuarios (roles de tienda y logistica)
    { patron: /^\/api\/v1\/auth\/crear-usuario$/, metodos: ['POST'], roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN] },
    { patron: /^\/api\/v1\/empresas$/, metodos: ['POST'], roles: [ROLES.EMPRESARIO] },
    { patron: /^\/api\/v1\/empresas\/[^/]+\/logistica$/, metodos: ['PATCH'], roles: [ROLES.EMPRESARIO] },
    {
      patron: /^\/api\/v1\/empresas\/[^/]+\/perfil$/,
      metodos: ['PATCH'],
      roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO],
    },
    { patron: /^\/api\/v1\/empresas\/[^/]+\/vendedores$/, metodos: ['POST', 'GET'], roles: [ROLES.EMPRESARIO] },
    {
      patron: /^\/api\/v1\/empresas\/[^/]+\/invitaciones\/vendedores(\/|$)/,
      metodos: ['POST', 'DELETE'],
      roles: [ROLES.EMPRESARIO],
    },
    // Cambio de slug: solo el empresario; el servicio exige membresia activa.
    { patron: /^\/api\/v1\/empresas\/[^/]+\/slug$/, metodos: ['PATCH'], roles: [ROLES.EMPRESARIO] },
    // GET /empresas/:id acepta UUID o slug, pero sigue siendo privado: solo
    // miembros o administradores. No debe existir una politica publica aqui.
    { patron: /^\/api\/v1\/empresas\//, metodos: ['*'], roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR] },
    // autenticado
    { patron: /^\/api\/v1\/auth\/me$/, metodos: ['GET'], roles: null }, // exige token (se valida abajo)
    // logout: exige un access token valido (cualquier rol); revoca el refresh (R-GW-4)
    { patron: /^\/api\/v1\/auth\/logout$/, metodos: ['POST'], roles: Object.values(ROLES) },
    // catálogo: escritura admin (el super_admin opera globalmente; para crear
    // debe indicar la empresa en el cuerpo)
    { patron: /^\/api\/v1\/catalog\/productos/, metodos: ['POST', 'PATCH', 'PUT', 'DELETE'], roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO] },
    // vendedores
    {
      patron: /^\/api\/v1\/vendedores\/(me\/ventas|me\/liquidaciones)$/,
      metodos: ['GET'],
      roles: [ROLES.VENDEDOR],
    },
    { patron: /^\/api\/v1\/empresarios\/me\/liquidaciones$/, metodos: ['GET'], roles: [ROLES.EMPRESARIO] },
    { patron: /^\/api\/v1\/empresarios\/me\/ventas$/, metodos: ['GET'], roles: [ROLES.EMPRESARIO] },
    { patron: /^\/api\/v1\/empresarios\/me\/tiendas$/, metodos: ['GET'], roles: [ROLES.EMPRESARIO] },
    { patron: /^\/api\/v1\/empresarios\/me\/envios(\/|$)/, metodos: ['GET', 'POST'], roles: [ROLES.EMPRESARIO] },
    { patron: /^\/api\/v1\/envios\/[^/]+\/estado$/, metodos: ['PATCH'], roles: [ROLES.SUPER_ADMIN, ...ROLES_LOGISTICA] },
    {
      patron: /^\/api\/v1\/vendedores\//,
      metodos: ['GET', 'POST', 'PATCH', 'PUT', 'DELETE'],
      roles: [ROLES.VENDEDOR, ROLES.EMPRESARIO],
    },
    // pedidos
    { patron: /^\/api\/v1\/orders$/, metodos: ['POST'], roles: [ROLES.COMPRADOR] },
    { patron: /^\/api\/v1\/orders$/, metodos: ['GET'], roles: [ROLES.COMPRADOR] },
    {
      patron: /^\/api\/v1\/orders\/[^/]+\/estado$/,
      metodos: ['PATCH', 'POST'],
      roles: [ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.LOGISTICA],
    },
    // Pedidos: reglas específicas antes de cualquier catch-all. El gateway
    // resuelve por primera coincidencia (find), así que una regla genérica
    // como /orders/ expondría las rutas admin y de tienda a otros roles
    // (R-GW-1, R-GW-3).
    { patron: /^\/api\/v1\/orders\/admin\/todas$/, metodos: ['GET'], roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN] },
    { patron: /^\/api\/v1\/orders\/vendedor\/mis-ventas$/, metodos: ['GET'], roles: [ROLES.VENDEDOR] },
    {
      patron: /^\/api\/v1\/orders\/vendedor\/comisiones$/,
      metodos: ['GET'],
      roles: [ROLES.SUPER_ADMIN, ROLES.VENDEDOR, ROLES.ADMIN],
    },
    {
      patron: /^\/api\/v1\/orders\/tienda\/[^/]+$/,
      metodos: ['GET'],
      roles: [ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR, ROLES.ADMIN],
    },
    {
      patron: /^\/api\/v1\/orders\/empresa$/,
      metodos: ['GET'],
      roles: [ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN],
    },
    {
      patron: /^\/api\/v1\/orders\/[^/]+\/timeline$/,
      metodos: ['GET'],
      roles: [ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.COMPRADOR, ROLES.VENDEDOR, ROLES.ADMIN],
    },
    {
      patron: /^\/api\/v1\/orders\/[^/]+$/,
      metodos: ['GET'],
      roles: [ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.COMPRADOR, ROLES.VENDEDOR, ROLES.ADMIN],
    },
    // carrito (RN-05): solo el comprador autenticado
    {
      patron: /^\/api\/v1\/carrito/,
      metodos: ['GET', 'POST', 'PATCH', 'DELETE'],
      roles: [ROLES.COMPRADOR],
    },
  // Imágenes públicas estáticas (ADR-28): sin token; el UUID es la capacidad.
  {
    patron: /^\/api\/v1\/storage\/public\/objetos\/[^/]+$/,
    metodos: ['GET'],
    roles: null,
  },
  // la configuracion de cuota del bucket es administrativa (Tabla 21)
  {
    patron: /^\/api\/v1\/storage\/bucket$/,
    metodos: ['POST'],
    roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN],
  },
  // almacenamiento de objetos por tenant (ADR-24): cualquier usuario autenticado
  // con tenant; el aislamiento real lo garantiza el bucket + el filtro por tenant.
  {
    patron: /^\/api\/v1\/storage\//,
    metodos: ['*'],
    roles: Object.values(ROLES),
  },
// admin
  {
    patron: /^\/api\/v1\/admin\/inventario/,
    metodos: ['GET', 'POST', 'PATCH'],
    roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN],
  },
  {
    patron: /^\/api\/v1\/admin\//,
    metodos: ['*'],
    roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN],
  },
  // Operación de comercio del workspace: empresa y vendedor comparten recursos
  // (bancos, créditos, clientes, códigos) y la empresa administra caja/compras.
  {
    patron: /^\/api\/v1\/comercio\//,
    metodos: ['*'],
    roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR],
  },
  // finanzas y regimen fiscal (contabilidad, comprobantes, proyecciones)
  {
    patron: /^\/api\/v1\/finanzas\//,
    metodos: ['*'],
    roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN],
  },
  // field-service: logistica de campo (app-test) — roles de logistica
  {
    patron: /^\/api\/v1\/field\//,
    metodos: ['*'],
    roles: [ROLES.SUPER_ADMIN, ...ROLES_LOGISTICA],
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
    roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN],
  },
  {
    patron: /^\/api\/v1\/inteligencia\/ventas\/registrar$/,
    metodos: ['POST'],
    roles: [ROLES.VENDEDOR, ROLES.COMPRADOR, ...ROLES_LOGISTICA],
  },
  // Deny-by-default: una ruta /api/v1 no declarada no queda abierta a cualquier
  // token valido. Las rutas publicas deben declarar roles: null antes de esta
  // regla; las protegidas, su lista de roles (R-GW-1, R-GW-3).
  {
    patron: /^\/api\/v1\//,
    metodos: ['*'],
    roles: [],
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
      if (payload.participacion_porcentaje !== undefined) req.headers[PARTICIPACION_HEADER] = String(payload.participacion_porcentaje);

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
