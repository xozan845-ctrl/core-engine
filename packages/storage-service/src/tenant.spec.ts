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

  it('debe rechazar el acceso sin tenant', () => {
    expect(() =>
      requerirTenant({ user_id: 'u1', email: 'u@core-engine.test', rol: ROLES.COMPRADOR }),
    ).toThrow(DomainError);
    expect(() => requerirTenant(null)).toThrow(/TENANT_REQUERIDO|tenant/);
    expect(() => requerirTenant(undefined)).toThrow(/TENANT_REQUERIDO|tenant/);
  });
});
