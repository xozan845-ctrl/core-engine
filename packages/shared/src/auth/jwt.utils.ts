import { Logger } from '../logging/logger';
import { ROLES, Rol } from '../constants';

export interface UsuarioContexto {
  user_id: string;
  email: string;
  rol: Rol;
  nombre?: string;
  /** Tenant (organizacion) al que pertenece el usuario; eje de multi-tenancy. */
  tenant_id?: string;
  /** Id de la ficha de personal (logistica de campo), si aplica. */
  personal_id?: string;
}

export interface TokenPayload {
  sub: string;
  email: string;
  rol: Rol;
  tipo: 'access' | 'refresh';
  nombre?: string;
  tenant_id?: string;
  personal_id?: string;
}

const jwt = (() => {
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    return require('jsonwebtoken');
  } catch {
    return undefined;
  }
})();

/**
 * Firma y verificacion JWT (HS256, Supabase-compatible).
 * Access: corta duracion (900s); refresh: 7 dias (doc 4.3).
 */
export function firmarToken(payload: TokenPayload, secreto: string, ttl: string): string {
  if (!jwt) throw new Error('jsonwebtoken no disponible.');
  return jwt.sign(payload, secreto, { expiresIn: ttl, algorithm: 'HS256' });
}

export function verificarToken<T>(token: string, secreto: string): T {
  if (!jwt) throw new Error('jsonwebtoken no disponible.');
  return jwt.verify(token, secreto, { algorithms: ['HS256'] }) as T;
}

export function crearAccessToken(
  usuario: { id: string; email: string; rol: Rol; nombre?: string; tenant_id?: string; personal_id?: string },
  secreto: string,
  ttl: string,
): string {
  return firmarToken(
    {
      sub: usuario.id,
      email: usuario.email,
      rol: usuario.rol,
      tipo: 'access',
      nombre: usuario.nombre,
      tenant_id: usuario.tenant_id,
      personal_id: usuario.personal_id,
    },
    secreto,
    ttl,
  );
}

export function crearRefreshToken(
  usuario: { id: string; email: string; rol: Rol; nombre?: string; tenant_id?: string; personal_id?: string },
  secreto: string,
  ttl: string,
): string {
  return firmarToken(
    {
      sub: usuario.id,
      email: usuario.email,
      rol: usuario.rol,
      tipo: 'refresh',
      nombre: usuario.nombre,
      tenant_id: usuario.tenant_id,
      personal_id: usuario.personal_id,
    },
    secreto,
    ttl,
  );
}

export function esRol(valor: string): valor is Rol {
  return Object.values(ROLES).includes(valor as Rol);
}

/**
 * Lee el contexto del usuario inyectado por el API Gateway
 * (cabeceras X-User-*; los servicios confian en el gateway y en RLS).
 */
export function usuarioDesdeHeaders(headers: Record<string, string | string[] | undefined>): UsuarioContexto | null {
  const get = (k: string): string | undefined => {
    const v = headers[k];
    return Array.isArray(v) ? v[0] : v;
  };
  const user_id = get('x-user-id');
  const email = get('x-user-email');
  const rol = get('x-user-rol');
  if (!user_id || !rol || !esRol(rol)) return null;
  const tenant_id = get('x-tenant');
  const personal_id = get('x-user-personal');
  const nombre = get('x-user-nombre');
  return {
    user_id,
    email: email ?? '',
    rol,
    ...(nombre ? { nombre } : {}),
    ...(tenant_id ? { tenant_id } : {}),
    ...(personal_id ? { personal_id } : {}),
  };
}

/** Helper de depuracion para el contexto del gateway. */
export function logErrorContexto(logger: ReturnType<typeof Logger.create>, err: unknown): void {
  logger.error({
    msg: 'Error en auth',
    err: err instanceof Error ? err.message : String(err),
  });
}