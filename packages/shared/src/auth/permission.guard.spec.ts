import { ExecutionContext } from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { describe, expect, it, vi } from 'vitest';
import { ForbiddenError, UnauthorizedError } from '../errors';
import { PERMISSIONS_KEY, PermisosGuard } from './service-auth';

const contexto = (headers: Record<string, string>, _permisos: string[] = []): ExecutionContext => ({
  getHandler: vi.fn(),
  getClass: vi.fn(),
  switchToHttp: () => ({ getRequest: () => ({ headers }) }),
} as unknown as ExecutionContext);

const reflector = (permisos: string[]): Reflector => ({
  getAllAndMerge: vi.fn((key: string) => key === PERMISSIONS_KEY ? permisos : []),
} as unknown as Reflector);

describe('PermisosGuard', () => {
  it('no consulta roles legacy si la ruta no declara permisos', async () => {
    const pg = { query: vi.fn() };
    const guard = new PermisosGuard(pg as never, reflector([]));

    await expect(guard.canActivate(contexto({}))).resolves.toBe(true);
    expect(pg.query).not.toHaveBeenCalled();
  });

  it('rechaza una ruta protegida sin contexto', async () => {
    const guard = new PermisosGuard({ query: vi.fn() } as never, reflector(['catalogo.leer']));

    await expect(guard.canActivate(contexto({}))).rejects.toBeInstanceOf(UnauthorizedError);
  });

  it('deja pasar roles fijos sin consultar permisos configurables', async () => {
    const pg = { query: vi.fn() };
    const guard = new PermisosGuard(pg as never, reflector(['catalogo.leer']));
    const headers = { 'x-user-id': 'u-1', 'x-user-rol': 'vendedor' };

    await expect(guard.canActivate(contexto(headers))).resolves.toBe(true);
    expect(pg.query).not.toHaveBeenCalled();
  });

  it('exige todas las acciones del rol configurable dentro del tenant', async () => {
    const pg = { query: vi.fn().mockResolvedValue([{ permisos: { acciones: ['catalogo.leer', 'ordenes.crear'] } }]) };
    const guard = new PermisosGuard(pg as never, reflector(['catalogo.leer', 'ordenes.crear']));
    const headers = {
      'x-user-id': 'u-1', 'x-user-rol': 'operativo', 'x-user-rol-empresa': 'r-1', 'x-tenant': 'e-1',
    };

    await expect(guard.canActivate(contexto(headers))).resolves.toBe(true);
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('roles_empresa'), ['r-1', 'u-1', 'e-1']);
  });

  it('rechaza permisos faltantes o membresía no vigente', async () => {
    const pg = { query: vi.fn().mockResolvedValue([{ permisos: { acciones: ['catalogo.leer'] } }]) };
    const guard = new PermisosGuard(pg as never, reflector(['catalogo.leer', 'ordenes.crear']));
    const headers = {
      'x-user-id': 'u-1', 'x-user-rol': 'operativo', 'x-user-rol-empresa': 'r-1', 'x-tenant': 'e-1',
    };

    await expect(guard.canActivate(contexto(headers))).rejects.toBeInstanceOf(ForbiddenError);
    pg.query.mockResolvedValue([]);
    await expect(guard.canActivate(contexto(headers))).rejects.toBeInstanceOf(ForbiddenError);
  });
});
