import type { Mock } from 'vitest';
import 'reflect-metadata';
import {
  EstadoOrdenController,
  EnviosAdminController,
  EnviosEmpresaController,
  EnviosFlotaController,
} from './estado-orden.controller';
import type { EnviosService } from './envios.service';
import { DomainError, ROLES } from '@core/shared';

const orden = { id: 'o-1', estado: 'enviada' };
const envio = { id: 'e-1', order_id: 'o-1', estado: 'en_preparacion' };
const logistica = { user_id: 'u-l', email: 'l@core.local', rol: ROLES.LOGISTICA };
const empresario = { user_id: 'u-e', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 'empresa-1' };

function fabricar(): { envios: Record<string, Mock>; controller: EstadoOrdenController } {
  const envios = { avanzarEstadoOrden: vi.fn(), listar: vi.fn() };
  const controller = new EstadoOrdenController(envios as unknown as EnviosService);
  return { envios, controller };
}

describe('EstadoOrdenController (api/v1/orders/:id/estado, Tabla 21, R-U-10/11)', () => {
  it('debe avanzar el estado delegando id, estado y motivo', async () => {
    const { envios, controller } = fabricar();
    envios.avanzarEstadoOrden.mockResolvedValue(orden);
    await expect(controller.avanzar('o-1', { estado: 'enviada', motivo: 'despacho' } as never, logistica as never)).resolves.toEqual(orden);
    expect(envios.avanzarEstadoOrden).toHaveBeenCalledWith('o-1', 'enviada', 'despacho');
  });

  it('debe lanzar DomainError ESTADO_INVALIDO cuando el estado no esta permitido', async () => {
    const { controller } = fabricar();
    let error: DomainError | undefined;
    try {
      await controller.avanzar('o-1', { estado: 'creada' } as never, logistica as never);
    } catch (e) {
      error = e as DomainError;
    }
    expect(error?.codigo).toBe('ESTADO_INVALIDO');
  });

  it('debe exigir rol admin o logistica', () => {
    expect(Reflect.getMetadata('roles_requeridos', EstadoOrdenController.prototype.avanzar)).toEqual([
      ROLES.ADMIN,
      ROLES.LOGISTICA,
    ]);
  });
});

describe('EnviosAdminController (api/v1/admin/envios, R-U-10/11)', () => {
  it('debe listar las guias de despacho', async () => {
    const envios = { listar: vi.fn().mockResolvedValue([{ ...envio, monto: '1150.00' }]) };
    const controller = new EnviosAdminController(envios as unknown as EnviosService);
    await expect(controller.listar()).resolves.toEqual([{ ...envio, monto: '1150.00' }]);
    expect(envios.listar).toHaveBeenCalledTimes(1);
  });

  it('debe exigir rol admin', () => {
    expect(Reflect.getMetadata('roles_requeridos', EnviosAdminController.prototype.listar)).toEqual([ROLES.ADMIN]);
  });
});

describe('EnviosEmpresaController', () => {
  it('lista y asigna envios dentro de la empresa activa', async () => {
    const envios = {
      deEmpresa: vi.fn().mockResolvedValue([envio]),
      asignarProveedor: vi.fn().mockResolvedValue(envio),
    };
    const controller = new EnviosEmpresaController(envios as never);
    await expect(controller.listar(empresario as never)).resolves.toEqual([envio]);
    await expect(controller.asignar(
      'e-1', { proveedor_tipo: 'empresa' }, empresario as never,
    )).resolves.toEqual(envio);
    expect(envios.asignarProveedor).toHaveBeenCalledWith(
      'e-1', 'empresa-1', 'empresa', 'u-e',
    );
  });

  it('rechaza listar o asignar sin empresa activa', async () => {
    const controller = new EnviosEmpresaController({} as never);
    expect(() => controller.listar({} as never)).toThrow(DomainError);
    expect(() => controller.asignar('e-1', { proveedor_tipo: 'plataforma' }, {} as never))
      .toThrow(DomainError);
  });
});

describe('EnviosFlotaController', () => {
  it('avanza un envio dentro del tenant de la flota', async () => {
    const envios = { avanzarEstadoEnvio: vi.fn().mockResolvedValue(envio) };
    const controller = new EnviosFlotaController(envios as never);
    await expect(controller.avanzar(
      'e-1', { estado: 'entregada', motivo: 'recibido' },
      { ...logistica, tenant_id: 'flota-1' } as never,
    )).resolves.toEqual(envio);
    expect(envios.avanzarEstadoEnvio).toHaveBeenCalledWith(
      'e-1', 'flota-1', 'entregada', 'recibido',
    );
  });

  it('rechaza avanzar sin tenant de flota', () => {
    const controller = new EnviosFlotaController({} as never);
    expect(() => controller.avanzar('e-1', { estado: 'enviada' }, logistica as never))
      .toThrow(DomainError);
  });
});
