import type { Mock } from 'vitest';
import 'reflect-metadata';
import { CAPABILITIES_KEY, CAPACIDADES, ROLES } from '@core/shared';
import { SuscripcionesController } from './suscripciones.controller';
import type { SuscripcionesService } from './suscripciones.service';

const empresario = { user_id: 'u-1', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 't-1' };
const admin = { user_id: 'u-2', email: 'a@core.local', rol: ROLES.ADMIN };
const SUSCRIPCION = '22222222-2222-4222-8222-222222222222';
const OFERTA = '33333333-3333-4333-8333-333333333333';

function fabricar(): { servicio: Record<string, Mock>; controller: SuscripcionesController } {
  const servicio = {
    listar: vi.fn(),
    obtener: vi.fn(),
    crear: vi.fn(),
    pausar: vi.fn(),
    reanudar: vi.fn(),
    cancelar: vi.fn(),
    renovar: vi.fn(),
    listarEjecuciones: vi.fn(),
  };
  return {
    servicio,
    controller: new SuscripcionesController(servicio as unknown as SuscripcionesService),
  };
}

describe('SuscripcionesController', () => {
  it('exige la capacidad de suscripciones y roles administrativos', () => {
    expect(Reflect.getMetadata(CAPABILITIES_KEY, SuscripcionesController)).toEqual([
      CAPACIDADES.SUSCRIPCIONES,
    ]);
    expect(Reflect.getMetadata('roles_requeridos', SuscripcionesController)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.ADMIN,
      ROLES.EMPRESARIO,
    ]);
  });

  it('resuelve la empresa del empresario y exige empresa_id al admin', async () => {
    const { servicio, controller } = fabricar();
    servicio.listar.mockResolvedValue([]);
    await expect(controller.listar(undefined, undefined, undefined, undefined, empresario as never))
      .resolves.toEqual([]);
    expect(servicio.listar).toHaveBeenCalledWith('t-1', {
      estado: undefined,
      comprador_id: undefined,
      limite: undefined,
    });
    expect(() =>
      controller.listar(undefined, undefined, undefined, undefined, admin as never),
    ).toThrow(/empresa_id/);
  });

  it('delega alta, estados, renovacion y ejecuciones', async () => {
    const { servicio, controller } = fabricar();
    servicio.crear.mockResolvedValue({ id: SUSCRIPCION });
    servicio.pausar.mockResolvedValue({ id: SUSCRIPCION });
    servicio.reanudar.mockResolvedValue({ id: SUSCRIPCION });
    servicio.cancelar.mockResolvedValue({ id: SUSCRIPCION });
    servicio.renovar.mockResolvedValue({ order_id: 'o-1' });
    servicio.listarEjecuciones.mockResolvedValue([]);

    await controller.crear(
      { oferta_id: OFERTA, intervalo: 'mensual' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crear).toHaveBeenCalledWith('t-1', 'u-1', {
      oferta_id: OFERTA,
      intervalo: 'mensual',
    });

    await controller.pausar(SUSCRIPCION, undefined, empresario as never);
    await controller.reanudar(SUSCRIPCION, undefined, empresario as never);
    await controller.cancelar(SUSCRIPCION, undefined, empresario as never);
    expect(servicio.pausar).toHaveBeenCalledWith('t-1', SUSCRIPCION);
    expect(servicio.reanudar).toHaveBeenCalledWith('t-1', SUSCRIPCION);
    expect(servicio.cancelar).toHaveBeenCalledWith('t-1', SUSCRIPCION);

    await controller.renovar(SUSCRIPCION, undefined, empresario as never);
    expect(servicio.renovar).toHaveBeenCalledWith('t-1', SUSCRIPCION, true);

    await controller.ejecuciones(SUSCRIPCION, undefined, empresario as never);
    expect(servicio.listarEjecuciones).toHaveBeenCalledWith('t-1', SUSCRIPCION);
  });

  it('permite al admin indicar la empresa', async () => {
    const { servicio, controller } = fabricar();
    servicio.obtener.mockResolvedValue({ id: SUSCRIPCION });
    await controller.obtener(SUSCRIPCION, 'e-9', admin as never);
    expect(servicio.obtener).toHaveBeenCalledWith('e-9', SUSCRIPCION);
  });
});
