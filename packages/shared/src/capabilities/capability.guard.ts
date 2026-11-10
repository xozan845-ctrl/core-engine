import { CanActivate, ExecutionContext, Injectable } from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { CapabilityDisabledError, DomainError, ServiceUnavailableError, UnauthorizedError } from '../errors';
import { ROLES } from '../constants';
import { usuarioDesdeHeaders } from '../auth/jwt.utils';
import { CAPABILITIES_KEY } from './capability.decorator';
import { Capacidad } from './capability.constants';
import { capacidadesDeEmpresas } from '../clients/service-clients';

/**
 * Guard de negocio por tenant. El gateway solo propaga el contexto autenticado;
 * las capacidades se consultan a identity por su superficie interna (no se lee
 * el esquema `identity` desde cada servicio).
 */
@Injectable()
export class CapabilityGuard implements CanActivate {
  constructor(private readonly reflector = new Reflector()) {}

  async canActivate(ctx: ExecutionContext): Promise<boolean> {
    const requeridas = this.reflector.getAllAndMerge<Capacidad[]>(
      CAPABILITIES_KEY,
      [ctx.getHandler(), ctx.getClass()],
    );
    if (!requeridas || requeridas.length === 0) return true;

    const req = ctx.switchToHttp().getRequest();
    const usuario = usuarioDesdeHeaders(req.headers ?? {});
    if (!usuario) throw new UnauthorizedError('Se requiere un contexto autenticado valido.');

    if ([ROLES.SUPER_ADMIN, ROLES.ADMIN].includes(usuario.rol as never)) return true;
    if (!usuario.tenant_id) {
      throw new DomainError('TENANT_REQUERIDO', 'La operacion requiere un tenant activo.');
    }
    const tenantId = usuario.tenant_id;

    const filas = await capacidadesDeEmpresas([tenantId], requeridas);
    if (filas === null) throw new ServiceUnavailableError('identity-service');

    const activas = new Set(
      filas.filter((fila) => fila.activo && fila.empresa_id === tenantId).map((fila) => fila.capacidad_codigo),
    );
    const faltantes = requeridas.filter((capacidad) => !activas.has(capacidad));
    if (faltantes.length > 0) {
      throw new CapabilityDisabledError(faltantes, tenantId);
    }
    return true;
  }
}
