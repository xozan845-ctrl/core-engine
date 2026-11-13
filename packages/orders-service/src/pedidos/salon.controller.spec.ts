import type { Mock } from 'vitest';
import 'reflect-metadata';
import { CAPABILITIES_KEY, CAPACIDADES, ROLES } from '@core/shared';
import { SalonController } from './salon.controller';
import type { SalonService } from './salon.service';

const empresario = { user_id: 'u-1', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 't-1' };
const admin = { user_id: 'u-2', email: 'a@core.local', rol: ROLES.ADMIN };
const MESA = '22222222-2222-4222-8222-222222222222';
const COMANDA = '33333333-3333-4333-8333-333333333333';
const ORDEN = '44444444-4444-4444-8444-444444444444';

function fabricar(): { servicio: Record<string, Mock>; controller: SalonController } {
  const servicio = {
    listarMesas: vi.fn(),
    crearMesa: vi.fn(),
    actualizarMesa: vi.fn(),
    listarComandas: vi.fn(),
    crearComanda: vi.fn(),
    actualizarComanda: vi.fn(),
  };
  return { servicio, controller: new SalonController(servicio as unknown as SalonService) };
}

describe('SalonController', () => {
  it('exige la capacidad de mesas y roles administrativos', () => {
    expect(Reflect.getMetadata(CAPABILITIES_KEY, SalonController)).toEqual([
      CAPACIDADES.MESAS,
    ]);
    expect(Reflect.getMetadata('roles_requeridos', SalonController)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.ADMIN,
      ROLES.EMPRESARIO,
    ]);
  });

  it('resuelve la empresa y delega mesas', async () => {
    const { servicio, controller } = fabricar();
    servicio.listarMesas.mockResolvedValue([]);
    servicio.crearMesa.mockResolvedValue({ id: MESA });
    servicio.actualizarMesa.mockResolvedValue({ id: MESA });

    await expect(
      controller.listarMesas(undefined, 'salon', 'libre', empresario as never),
    ).resolves.toEqual([]);
    expect(servicio.listarMesas).toHaveBeenCalledWith('t-1', { zona: 'salon', estado: 'libre' });

    await controller.crearMesa(
      { codigo: 'M-01', nombre: 'Mesa 1' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crearMesa).toHaveBeenCalledWith('t-1', { codigo: 'M-01', nombre: 'Mesa 1' });

    await controller.actualizarMesa(MESA, { estado: 'ocupada' } as never, undefined, empresario as never);
    expect(servicio.actualizarMesa).toHaveBeenCalledWith('t-1', MESA, { estado: 'ocupada' });

    expect(() => controller.listarMesas(undefined, undefined, undefined, admin as never)).toThrow(
      /empresa_id/,
    );
  });

  it('delega comandas y permite al admin elegir empresa', async () => {
    const { servicio, controller } = fabricar();
    servicio.listarComandas.mockResolvedValue([]);
    servicio.crearComanda.mockResolvedValue({ id: COMANDA });
    servicio.actualizarComanda.mockResolvedValue({ id: COMANDA });

    await controller.listarComandas('e-9', 'pendiente', 'cocina', MESA, admin as never);
    expect(servicio.listarComandas).toHaveBeenCalledWith('e-9', {
      estado: 'pendiente',
      estacion: 'cocina',
      mesa_id: MESA,
    });

    await controller.crearComanda(
      { order_id: ORDEN, estacion: 'cocina' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crearComanda).toHaveBeenCalledWith('t-1', {
      order_id: ORDEN,
      estacion: 'cocina',
    });

    await controller.actualizarComanda(
      COMANDA,
      { estado: 'lista' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.actualizarComanda).toHaveBeenCalledWith('t-1', COMANDA, { estado: 'lista' });
  });
});
