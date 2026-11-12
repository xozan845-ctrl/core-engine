import 'reflect-metadata';
import type { ExecutionContext } from '@nestjs/common';
import type { Reflector } from '@nestjs/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { CapabilityGuard } from './capability.guard';
import { CAPACIDADES } from './capability.constants';
import {
  CapabilityDisabledError,
  DomainError,
  ServiceUnavailableError,
  UnauthorizedError,
} from '../errors';
import { capacidadesDeEmpresas } from '../clients/service-clients';

vi.mock('../clients/service-clients', () => ({
  capacidadesDeEmpresas: vi.fn(),
}));

const capacidadesMock = vi.mocked(capacidadesDeEmpresas);

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

const headersEmpresario = {
  'x-user-id': 'u-1',
  'x-user-rol': 'empresario',
  'x-tenant': 'e-1',
};

describe('CapabilityGuard (ADR-0001)', () => {
  beforeEach(() => {
    capacidadesMock.mockReset();
  });

  it('permite rutas que no requieren capacidades', async () => {
    const guard = new CapabilityGuard(reflectorCon(undefined));
    await expect(guard.canActivate(contextoCon({}))).resolves.toBe(true);
    expect(capacidadesMock).not.toHaveBeenCalled();
  });

  it('rechaza una ruta con capacidades sin contexto autenticado', async () => {
    const guard = new CapabilityGuard(reflectorCon([CAPACIDADES.AGENDA_RECURSOS]));
    await expect(guard.canActivate(contextoCon({}))).rejects.toBeInstanceOf(UnauthorizedError);
  });

  it('exige tenant para roles no globales', async () => {
    const guard = new CapabilityGuard(reflectorCon([CAPACIDADES.CATALOGO_ITEMS]));
    await expect(
      guard.canActivate(contextoCon({ 'x-user-id': 'u-1', 'x-user-rol': 'empresario' })),
    ).rejects.toMatchObject({ codigo: 'TENANT_REQUERIDO' });
  });

  it('permite a super_admin sin consultar capacidades', async () => {
    const guard = new CapabilityGuard(reflectorCon([CAPACIDADES.CATALOGO_ITEMS]));
    await expect(
      guard.canActivate(
        contextoCon({ 'x-user-id': 'u-1', 'x-user-rol': 'super_admin', 'x-tenant': 'e-1' }),
      ),
    ).resolves.toBe(true);
    expect(capacidadesMock).not.toHaveBeenCalled();
  });

  it('falla cerrado si identity no responde', async () => {
    capacidadesMock.mockResolvedValue(null);
    const guard = new CapabilityGuard(reflectorCon([CAPACIDADES.CATALOGO_ITEMS]));
    await expect(guard.canActivate(contextoCon(headersEmpresario))).rejects.toBeInstanceOf(
      ServiceUnavailableError,
    );
  });

  it('rechaza una capacidad inactiva con un error estable', async () => {
    capacidadesMock.mockResolvedValue([
      { empresa_id: 'e-1', capacidad_codigo: CAPACIDADES.CATALOGO_ITEMS, activo: false },
    ]);
    const guard = new CapabilityGuard(reflectorCon([CAPACIDADES.CATALOGO_ITEMS]));
    await expect(guard.canActivate(contextoCon(headersEmpresario))).rejects.toBeInstanceOf(
      CapabilityDisabledError,
    );
  });

  it('ignora capacidades de otra empresa', async () => {
    capacidadesMock.mockResolvedValue([
      { empresa_id: 'e-otra', capacidad_codigo: CAPACIDADES.CATALOGO_ITEMS, activo: true },
    ]);
    const guard = new CapabilityGuard(reflectorCon([CAPACIDADES.CATALOGO_ITEMS]));
    await expect(guard.canActivate(contextoCon(headersEmpresario))).rejects.toBeInstanceOf(
      CapabilityDisabledError,
    );
  });

  it('permite cuando todas las capacidades requeridas están activas', async () => {
    capacidadesMock.mockResolvedValue([
      { empresa_id: 'e-1', capacidad_codigo: CAPACIDADES.CATALOGO_ITEMS, activo: true },
      { empresa_id: 'e-1', capacidad_codigo: CAPACIDADES.FACTURACION, activo: true },
    ]);
    const guard = new CapabilityGuard(
      reflectorCon([CAPACIDADES.CATALOGO_ITEMS, CAPACIDADES.FACTURACION]),
    );
    await expect(guard.canActivate(contextoCon(headersEmpresario))).resolves.toBe(true);
  });

  it('propaga DomainError si falta el tenant en una ruta protegida', async () => {
    const guard = new CapabilityGuard(reflectorCon([CAPACIDADES.CATALOGO_ITEMS]));
    await expect(
      guard.canActivate(contextoCon({ 'x-user-id': 'u-1', 'x-user-rol': 'vendedor' })),
    ).rejects.toBeInstanceOf(DomainError);
  });
});
