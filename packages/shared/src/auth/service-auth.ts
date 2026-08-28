import {
  CanActivate,
  createParamDecorator,
  ExecutionContext,
  Injectable,
  SetMetadata,
} from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { ForbiddenError, UnauthorizedError } from '../errors';
import { Rol } from '../constants';
import { usuarioDesdeHeaders, UsuarioContexto } from './jwt.utils';

export const ROLES_KEY = 'roles_requeridos';
/** @Roles(R.ADMIN) marca un endpoint como restringido por rol. */
export const Roles = (...roles: Rol[]) => SetMetadata(ROLES_KEY, roles);
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
      throw new ForbiddenError(`Rol "${usuario.rol}" no tiene acceso a este recurso.`);
    }
    return true;
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