import {
  CanActivate,
  createParamDecorator,
  ExecutionContext,
  Injectable,
  SetMetadata,
} from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { ForbiddenError, UnauthorizedError } from '../errors';
import { PgService } from '../db/pg.service';
import { Rol } from '../constants';
import { usuarioDesdeHeaders, UsuarioContexto } from './jwt.utils';

export const ROLES_KEY = 'roles_requeridos';
/** @Roles(R.ADMIN) marca un endpoint como restringido por rol. */
export const Roles = (...roles: Rol[]) => SetMetadata(ROLES_KEY, roles);
export const PERMISSIONS_KEY = 'permisos_requeridos';
/** @Permisos('catalogo.leer') habilita la ruta para roles configurables. */
export const Permisos = (...permisos: string[]) => SetMetadata(PERMISSIONS_KEY, permisos);
export const AUTENTICADO_KEY = 'requiere_autenticacion';
export const RequiereAutenticacion = () => SetMetadata(AUTENTICADO_KEY, true);

export const UsuarioActual = createParamDecorator(
  (_: unknown, ctx: ExecutionContext): UsuarioContexto | null => {
    const req = ctx.switchToHttp().getRequest();
    return usuarioDesdeHeaders(req.headers ?? {});
  },
);

/**
 * Guard del lado del servicio: valida el contexto inyectado por el gateway
 * (quien verifico el JWT). La autorizacion a nivel de fila la provee RLS.
 */
@Injectable()
export class RolesGuard implements CanActivate {
  constructor(private readonly reflector = new Reflector()) {}

  canActivate(ctx: ExecutionContext): boolean {
    const requeridos = this.reflector.getAllAndOverride<Rol[] | undefined>(ROLES_KEY, [
      ctx.getHandler(),
      ctx.getClass(),
    ]);
    if (!requeridos || requeridos.length === 0) return true;
    const req = ctx.switchToHttp().getRequest();
    const usuario = usuarioDesdeHeaders(req.headers ?? {});
    if (!usuario) {
      throw new UnauthorizedError('Se requiere un token valido.');
    }
    if (!requeridos.includes(usuario.rol)) {
      const permisos = usuario.rol_empresa_id
        ? this.reflector.getAllAndMerge<string[]>(PERMISSIONS_KEY as never, [
          ctx.getHandler(),
          ctx.getClass(),
        ]) as string[]
        : [];
      if (!(usuario.rol_empresa_id && permisos.length > 0)) {
        throw new ForbiddenError(`Rol "${usuario.rol}" no tiene acceso a este recurso.`);
      }
    }
    return true;
  }
}

interface PermisosRolRow {
  permisos: unknown;
}

/**
 * Autoriza acciones declaradas por un rol configurable de empresa.
 * Los roles fijos se validan con RolesGuard y no consultan esta tabla.
 */
@Injectable()
export class PermisosGuard implements CanActivate {
  constructor(
    private readonly pg: PgService,
    private readonly reflector = new Reflector(),
  ) {}

  async canActivate(ctx: ExecutionContext): Promise<boolean> {
    const requeridos = this.reflector.getAllAndMerge<string[]>(PERMISSIONS_KEY as never, [
      ctx.getHandler(),
      ctx.getClass(),
    ]) as string[];
    if (requeridos.length === 0) return true;

    const req = ctx.switchToHttp().getRequest();
    const usuario = usuarioDesdeHeaders(req.headers ?? {});
    if (!usuario) throw new UnauthorizedError('Se requiere un token valido.');
    if (!usuario.rol_empresa_id) return true;
    if (!usuario.tenant_id) {
      throw new ForbiddenError('El rol configurable requiere un tenant activo.');
    }

    const filas = await this.pg.query<PermisosRolRow>(
      `SELECT r.permisos
       FROM identity.roles_empresa r
       JOIN identity.asignaciones_personal a
         ON a.rol_empresa_id = r.id
        AND a.usuario_id = $2
        AND a.estado = 'activa'
       WHERE r.id = $1
         AND r.empresa_id = $3
         AND r.activo = true`,
      [usuario.rol_empresa_id, usuario.user_id, usuario.tenant_id],
    );
    const permisos = filas[0]?.permisos;
    const acciones = this.acciones(permisos);
    const faltantes = requeridos.filter((permiso) => !acciones.has(permiso));
    if (faltantes.length > 0) {
      throw new ForbiddenError(`El rol configurable no tiene permisos: ${faltantes.join(', ')}.`);
    }
    return true;
  }

  private acciones(permisos: unknown): Set<string> {
    if (!permisos || typeof permisos !== 'object' || Array.isArray(permisos)) return new Set();
    const acciones = (permisos as { acciones?: unknown }).acciones;
    if (!Array.isArray(acciones)) return new Set();
    return new Set(acciones.filter((accion): accion is string => typeof accion === 'string'));
  }
}

/** Clave interna entre servicios (llamadas back-to-back, nunca expuesta al publico). */
export class ClaveInternaGuard implements CanActivate {
  canActivate(ctx: ExecutionContext): boolean {
    const req = ctx.switchToHttp().getRequest();
    const clave = req.headers?.['x-internal-key'];
    if (!clave || clave !== process.env.INTERNAL_API_KEY) {
      throw new ForbiddenError('Clave interna invalida.');
    }
    return true;
  }
}
