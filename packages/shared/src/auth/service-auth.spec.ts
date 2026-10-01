import 'reflect-metadata';
import type { ExecutionContext } from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { RolesGuard, ClaveInternaGuard, Roles, ROLES_KEY } from './service-auth';
import { UnauthorizedError, ForbiddenError } from '../errors';
import { ROLES } from '../constants';

function contextoConHeaders(headers: Record<string, string>): ExecutionContext {
  return {
    switchToHttp: () => ({ getRequest: () => ({ headers }) }),
    getHandler: () => ({}),
    getClass: () => ({}),
  } as unknown as ExecutionContext;
}

function reflectorCon(requeridos: unknown): { getAllAndOverride: () => unknown } {
  return { getAllAndOverride: () => requeridos };
}

describe('RolesGuard (contexto del gateway, R-U-15)', () => {
  it('debe permitir cuando el endpoint no declara roles requeridos', () => {
    const guard = new RolesGuard(reflectorCon(undefined) as unknown as Reflector);
    expect(guard.canActivate(contextoConHeaders({}))).toBe(true);
  });

  it('debe rechazar con NO_AUTORIZADO cuando falta el contexto', () => {
    const guard = new RolesGuard(reflectorCon([ROLES.ADMIN]) as unknown as Reflector);
    expect(() => guard.canActivate(contextoConHeaders({}))).toThrow(UnauthorizedError);
  });

  it('debe rechazar con ACCESO_DENEGADO cuando el rol no esta en los requeridos', () => {
    const guard = new RolesGuard(reflectorCon([ROLES.ADMIN]) as unknown as Reflector);
    expect(() =>
      guard.canActivate(contextoConHeaders({ 'x-user-id': 'u-1', 'x-user-rol': ROLES.VENDEDOR })),
    ).toThrow(ForbiddenError);
  });

  it('debe permitir cuando el rol del contexto esta en los requeridos', () => {
    const guard = new RolesGuard(reflectorCon([ROLES.ADMIN, ROLES.VENDEDOR]) as unknown as Reflector);
    expect(guard.canActivate(contextoConHeaders({ 'x-user-id': 'u-1', 'x-user-rol': ROLES.VENDEDOR }))).toBe(true);
  });
});

describe('ClaveInternaGuard (llamadas entre servicios)', () => {
  const claveOriginal = process.env.INTERNAL_API_KEY;

  afterEach(() => {
    process.env.INTERNAL_API_KEY = claveOriginal;
  });

  it('debe permitir cuando la cabecera x-internal-key coincide', () => {
    process.env.INTERNAL_API_KEY = 'secreto-interno';
    const guard = new ClaveInternaGuard();
    expect(guard.canActivate(contextoConHeaders({ 'x-internal-key': 'secreto-interno' }))).toBe(true);
  });

  it('debe rechazar con ACCESO_DENEGADO cuando la clave falta o no coincide', () => {
    process.env.INTERNAL_API_KEY = 'secreto-interno';
    const guard = new ClaveInternaGuard();
    expect(() => guard.canActivate(contextoConHeaders({}))).toThrow(ForbiddenError);
    expect(() => guard.canActivate(contextoConHeaders({ 'x-internal-key': 'otra' }))).toThrow(ForbiddenError);
  });
});

describe('Roles (metadata de requerimiento)', () => {
  it('debe registrar los roles declarados como metadata del handler', () => {
    const reflector = new Reflector();
    class ControladorPrueba {
      @Roles(ROLES.ADMIN, ROLES.VENDEDOR)
      ruta(): void {
        /* noop */
      }
    }
    const metadata = reflector.get(ROLES_KEY, ControladorPrueba.prototype.ruta);
    expect(metadata).toEqual([ROLES.ADMIN, ROLES.VENDEDOR]);
  });
});