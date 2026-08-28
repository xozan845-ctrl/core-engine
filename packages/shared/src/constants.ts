export const ROLES = {
  ADMIN: 'admin',
  VENDEDOR: 'vendedor',
  COMPRADOR: 'comprador',
  LOGISTICA: 'logistica',
  COORDINADOR: 'coordinador',
  SUPERVISOR: 'supervisor',
  OPERATIVO: 'operativo',
} as const;

export type Rol = (typeof ROLES)[keyof typeof ROLES];

export const ROLES_REGISTRABLES: Rol[] = [ROLES.VENDEDOR, ROLES.COMPRADOR];

/** Roles de la aplicacion de logistica de campo (app-test). */
export const ROLES_LOGISTICA: Rol[] = [
  ROLES.ADMIN,
  ROLES.LOGISTICA,
  ROLES.COORDINADOR,
  ROLES.SUPERVISOR,
  ROLES.OPERATIVO,
];

/** Cabeceras de contexto propagadas por el gateway a los servicios. */
export const TENANT_HEADER = 'x-tenant';
export const PERSONAL_HEADER = 'x-user-personal';

/**
 * RN-01: el vendedor fija un margen entre 0 y 90 % (expresado en enteros, ej. 15 = 15 %).
 * Precio de venta final = base x (1 + margen).
 */
export const MARGEN_MINIMO = 0;
export const MARGEN_MAXIMO = 90;

/** RN-04: comision de la plataforma por venta (por defecto 12 %). */
export const COMMISSION_RATE_DEFAULT = 0.12;

/** RN-05: el carrito expira tras 30 minutos de inactividad. */
export const CARRITO_EXPIRACION_MS = 30 * 60 * 1000;

/** RN-07: liquidacion quincenal (dias 1 y 15). */
export const LIQUIDACION_DIAS = [1, 15];

export const ESTADOS_VISIBLES_PUBLICO = ['enviada', 'entregada'] as const;

export const JWT_ACCESS_TTL_DEFAULT = '900s';
export const JWT_REFRESH_TTL_DEFAULT = '7d';

export const PUERTOS = {
  GATEWAY: 8080,
  IDENTITY: 3001,
  CATALOG: 3002,
  STORES: 3003,
  ORDERS: 3004,
  LOGISTICS: 3005,
  COMMISSIONS: 3006,
  FINANCE: 3007,
  FIELD: 3008,
} as const;

export const NOMBRE_SERVICIOS = {
  GATEWAY: 'api-gateway',
  IDENTITY: 'identity-service',
  CATALOG: 'catalog-service',
  STORES: 'stores-service',
  ORDERS: 'orders-service',
  LOGISTICS: 'logistics-service',
  COMMISSIONS: 'commissions-service',
  FINANCE: 'finance-service',
  FIELD: 'field-service',
} as const;