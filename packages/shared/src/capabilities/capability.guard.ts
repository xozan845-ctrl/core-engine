import { CanActivate, ExecutionContext, Injectable } from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { PgService } from '../db/pg.service';
import { CapabilityDisabledError, DomainError, UnauthorizedError } from '../errors';
import { ROLES } from '../constants';
import { usuarioDesdeHeaders } from '../auth/jwt.utils';
import { CAPABILITIES_KEY } from './capability.decorator';
import { Capacidad } from './capability.constants';

interface CapacidadRow {
  capacidad_codigo: string;
  activo: boolean;
}

/**
 * Guard de negocio por tenant. El gateway solo propaga el contexto autenticado;
 * la empresa y sus capacidades se validan en el servicio que ejecuta la ruta.
 */
@Injectable()
export class CapabilityGuard implements CanActivate {
  constructor(
    private readonly pg: PgService,
    private readonly reflector = new Reflector(),
  ) {}

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

    const filas = await this.pg.query<CapacidadRow>(
      `SELECT capacidad_codigo, activo
       FROM identity.empresa_capacidades
       WHERE empresa_id = $1 AND capacidad_codigo = ANY($2::text[])`,
      [usuario.tenant_id, requeridas],
    );
    const activas = new Set(filas.filter((fila) => fila.activo).map((fila) => fila.capacidad_codigo));
    const faltantes = requeridas.filter((capacidad) => !activas.has(capacidad));
    if (faltantes.length > 0) {
      throw new CapabilityDisabledError(faltantes, usuario.tenant_id);
    }
    return true;
  }
}
