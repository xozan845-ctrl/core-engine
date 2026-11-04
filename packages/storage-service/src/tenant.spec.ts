import { DomainError, ROLES } from '@core/shared';
import { requerirTenant } from './tenant';

describe('requerirTenant (multitenancy del almacenamiento, ADR-24)', () => {
  it('debe devolver el tenant inyectado por el gateway', () => {
    expect(
      requerirTenant({
        user_id: 'u1',
        email: 'u@core-engine.test',
        rol: ROLES.VENDEDOR,
        tenant_id: 't1',
      }),
    ).toBe('t1');
  });

  it('debe usar PLATFORM_TENANT_ID para los roles globales sin tenant', () => {
    vi.stubEnv('PLATFORM_TENANT_ID', '00000000-0000-0000-0000-000000000002');
    expect(
      requerirTenant({ user_id: 'u1', email: 'admin@core-engine.test', rol: ROLES.ADMIN }),
    ).toBe('00000000-0000-0000-0000-000000000002');
    expect(
      requerirTenant({ user_id: 'u2', email: 'super@core-engine.test', rol: ROLES.SUPER_ADMIN }),
    ).toBe('00000000-0000-0000-0000-000000000002');
    vi.unstubAllEnvs();
  });

  it('no debe dar tenant de plataforma a roles no globales ni con un valor inválido', () => {
    vi.stubEnv('PLATFORM_TENANT_ID', 'no-es-uuid');
    expect(() =>
      requerirTenant({ user_id: 'u1', email: 'admin@core-engine.test', rol: ROLES.ADMIN }),
    ).toThrow(DomainError);
    vi.stubEnv('PLATFORM_TENANT_ID', '00000000-0000-0000-0000-000000000002');
    expect(() =>
      requerirTenant({ user_id: 'u2', email: 'v@core-engine.test', rol: ROLES.VENDEDOR }),
    ).toThrow(DomainError);
    vi.unstubAllEnvs();
  });

  it('debe rechazar el acceso sin tenant', () => {
    expect(() =>
      requerirTenant({ user_id: 'u1', email: 'u@core-engine.test', rol: ROLES.COMPRADOR }),
    ).toThrow(DomainError);
    expect(() => requerirTenant(null)).toThrow(/TENANT_REQUERIDO|tenant/);
    expect(() => requerirTenant(undefined)).toThrow(/TENANT_REQUERIDO|tenant/);
  });
});
