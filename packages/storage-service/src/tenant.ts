import { DomainError, UsuarioContexto } from '@core/shared';

/**
 * El almacenamiento es por tenant (bucket por organizacion, ADR-24): sin
 * `x-tenant` (que el gateway deriva del JWT emitido por identity-service) no
 * hay bucket destino. Los usuarios sin tenant no tienen almacenamiento propio.
 */
export function requerirTenant(usuario?: UsuarioContexto | null): string {
  if (!usuario?.tenant_id) {
    throw new DomainError(
      'TENANT_REQUERIDO',
      'El almacenamiento requiere un tenant (cabecera x-tenant del token).',
    );
  }
  return usuario.tenant_id;
}
