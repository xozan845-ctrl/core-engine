import 'reflect-metadata';
import type { ExecutionContext } from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { CapabilityGuard } from './capability.guard';
import { CAPACIDADES } from './capability.constants';
import { CapabilityDisabledError, UnauthorizedError } from '../errors';

function contextoCon(headers: Record<string, string>): ExecutionContext {
  return {
    switchToHttp: () => ({ getRequest: () => ({ headers }) }),
    getHandler: () => ({}),
    getClass: () => ({}),
  } as unknown as ExecutionContext;
}

function reflectorCon(requeridas: string[] | undefined): Reflector {
  return { getAllAndMerge: () => requeridas ?? [] } as unknown as Reflector;
}

describe('CapabilityGuard (ADR-0001)', () => {
  it('permite rutas que no requieren capacidades', async () => {
    const pg = { query: vi.fn() };
    const guard = new CapabilityGuard(pg as never, reflectorCon(undefined));
    await expect(guard.canActivate(contextoCon({}))).resolves.toBe(true);
    expect(pg.query).not.toHaveBeenCalled();
  });

  it('rechaza una ruta con capacidades sin contexto autenticado', async () => {
    const guard = new CapabilityGuard({ query: vi.fn() } as never, reflectorCon([CAPACIDADES.AGENDA_RECURSOS]));
    await expect(guard.canActivate(contextoCon({}))).rejects.toBeInstanceOf(UnauthorizedError);
  });

  it('rechaza una capacidad inactiva con un error estable', async () => {
    const pg = { query: vi.fn().mockResolvedValue([{ capacidad_codigo: CAPACIDADES.CATALOGO_ITEMS, activo: false }]) };
    const guard = new CapabilityGuard(pg as never, reflectorCon([CAPACIDADES.CATALOGO_ITEMS]));
    await expect(
      guard.canActivate(contextoCon({
        'x-user-id': 'u-1',
        'x-user-rol': 'empresario',
        'x-tenant': 'e-1',
      })),
    ).rejects.toBeInstanceOf(CapabilityDisabledError);
  });

  it('permite cuando todas las capacidades requeridas estan activas', async () => {
    const pg = {
      query: vi.fn().mockResolvedValue([
        { capacidad_codigo: CAPACIDADES.CATALOGO_ITEMS, activo: true },
        { capacidad_codigo: CAPACIDADES.FACTURACION, activo: true },
      ]),
    };
    const guard = new CapabilityGuard(pg as never, reflectorCon([CAPACIDADES.CATALOGO_ITEMS, CAPACIDADES.FACTURACION]));
    await expect(
      guard.canActivate(contextoCon({
        'x-user-id': 'u-1',
        'x-user-rol': 'empresario',
        'x-tenant': 'e-1',
      })),
    ).resolves.toBe(true);
  });
});
