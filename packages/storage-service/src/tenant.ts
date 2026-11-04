import { DomainError, ROLES, UsuarioContexto, esUuid } from '@core/shared';

/**
 * El almacenamiento es por tenant (bucket por organizacion, ADR-24): sin
 * `x-tenant` (que el gateway deriva del JWT emitido por identity-service) no
 * hay bucket destino. Los usuarios sin tenant no tienen almacenamiento propio.
 */
export function requerirTenant(usuario?: UsuarioContexto | null): string {
  if (usuario?.tenant_id) return usuario.tenant_id;

  // Los roles globales no tienen tenant en el JWT, pero necesitan un bucket
  // para los assets de la plataforma (fondo del sitio). El tenant se configura
  // explícitamente; sin él se mantiene el rechazo (no se inventa un bucket).
  const plataforma = (process.env.PLATFORM_TENANT_ID ?? '').trim();
  if (
    [ROLES.SUPER_ADMIN, ROLES.ADMIN].includes(usuario?.rol as never) &&
    esUuid(plataforma)
  ) {
    return plataforma;
  }

  throw new DomainError(
    'TENANT_REQUERIDO',
    'El almacenamiento requiere un tenant (cabecera x-tenant del token).',
  );
}
