import type { Mock } from 'vitest';
import 'reflect-metadata';
import { CAPABILITIES_KEY, CAPACIDADES, ROLES } from '@core/shared';
import { SeriesController } from './series.controller';
import type { SeriesService } from './series.service';

const empresario = { user_id: 'u-1', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 't-1' };
const admin = { user_id: 'u-2', email: 'a@core.local', rol: ROLES.ADMIN };
const ITEM = '22222222-2222-4222-8222-222222222222';
const SERIE = '33333333-3333-4333-8333-333333333333';

function fabricar(): { servicio: Record<string, Mock>; controller: SeriesController } {
  const servicio = {
    listar: vi.fn(),
    crear: vi.fn(),
    actualizar: vi.fn(),
    buscarPorSerial: vi.fn(),
    habilitarSerializacion: vi.fn(),
  };
  return { servicio, controller: new SeriesController(servicio as unknown as SeriesService) };
}

describe('SeriesController (catálogo)', () => {
  it('exige la capacidad de catálogo y roles administrativos', () => {
    expect(Reflect.getMetadata(CAPABILITIES_KEY, SeriesController)).toEqual([
      CAPACIDADES.CATALOGO_ITEMS,
    ]);
    expect(Reflect.getMetadata('roles_requeridos', SeriesController)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.ADMIN,
      ROLES.EMPRESARIO,
    ]);
  });

  it('resuelve la empresa del empresario y exige empresa_id al admin', async () => {
    const { servicio, controller } = fabricar();
    servicio.listar.mockResolvedValue([]);
    await expect(controller.listar(ITEM, undefined, empresario as never)).resolves.toEqual([]);
    expect(servicio.listar).toHaveBeenCalledWith('t-1', ITEM);
    expect(() => controller.listar(ITEM, undefined, admin as never)).toThrow(/empresa_id/);
    expect(() => controller.listar(ITEM, 'otra', empresario as never)).toThrow(
      /No puedes operar otra empresa/,
    );
  });

  it('delega alta, actualizacion y consulta por serial', async () => {
    const { servicio, controller } = fabricar();
    servicio.crear.mockResolvedValue({ id: SERIE });
    servicio.actualizar.mockResolvedValue({ id: SERIE });
    servicio.buscarPorSerial.mockResolvedValue({ serial: 'SN-1', garantia_vigente: true });

    await controller.crear(
      ITEM,
      { serial: 'SN-1' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crear).toHaveBeenCalledWith('t-1', ITEM, { serial: 'SN-1' });

    await controller.actualizar(
      SERIE,
      { estado: 'baja' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.actualizar).toHaveBeenCalledWith('t-1', SERIE, { estado: 'baja' });

    await controller.buscar('SN-1', undefined, empresario as never);
    expect(servicio.buscarPorSerial).toHaveBeenCalledWith('t-1', 'SN-1');
  });

  it('delega la activacion de serializacion y permite al admin elegir empresa', async () => {
    const { servicio, controller } = fabricar();
    servicio.habilitarSerializacion.mockResolvedValue({ item_id: ITEM, track_serial: true });
    await controller.habilitar(ITEM, { activo: true } as never, 'e-9', admin as never);
    expect(servicio.habilitarSerializacion).toHaveBeenCalledWith('e-9', ITEM, true);
  });
});
