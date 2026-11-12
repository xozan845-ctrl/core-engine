import type { Mock } from 'vitest';
import 'reflect-metadata';
import { CAPABILITIES_KEY, CAPACIDADES, ROLES } from '@core/shared';
import { UbicacionesController } from './ubicaciones.controller';
import type { UbicacionesService } from './ubicaciones.service';

const empresario = { user_id: 'u-1', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 't-1' };
const admin = { user_id: 'u-2', email: 'a@core.local', rol: ROLES.ADMIN };
const UBICACION = '33333333-3333-4333-8333-333333333333';
const ITEM = '22222222-2222-4222-8222-222222222222';

function fabricar(): { servicio: Record<string, Mock>; controller: UbicacionesController } {
  const servicio = {
    listar: vi.fn(),
    crear: vi.fn(),
    actualizar: vi.fn(),
    listarStock: vi.fn(),
    stockDeItem: vi.fn(),
    ajustarStock: vi.fn(),
    trasladar: vi.fn(),
  };
  return {
    servicio,
    controller: new UbicacionesController(servicio as unknown as UbicacionesService),
  };
}

describe('UbicacionesController (catálogo)', () => {
  it('exige la capacidad de inventario y roles administrativos', () => {
    expect(Reflect.getMetadata(CAPABILITIES_KEY, UbicacionesController)).toEqual([
      CAPACIDADES.INVENTARIO_STOCK,
    ]);
    expect(Reflect.getMetadata('roles_requeridos', UbicacionesController)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.ADMIN,
      ROLES.EMPRESARIO,
    ]);
  });

  it('resuelve la empresa del empresario y exige empresa_id al admin', async () => {
    const { servicio, controller } = fabricar();
    servicio.listar.mockResolvedValue([]);
    await expect(controller.listar(undefined, empresario as never)).resolves.toEqual([]);
    expect(servicio.listar).toHaveBeenCalledWith('t-1');
    expect(() => controller.listar(undefined, admin as never)).toThrow(/empresa_id/);
    expect(() => controller.listar('otra', empresario as never)).toThrow(
      /No puedes operar otra empresa/,
    );
  });

  it('delega el alta y la actualizacion de ubicaciones', async () => {
    const { servicio, controller } = fabricar();
    servicio.crear.mockResolvedValue({ id: UBICACION });
    servicio.actualizar.mockResolvedValue({ id: UBICACION });

    await controller.crear(
      { codigo: 'bodega', nombre: 'Bodega' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crear).toHaveBeenCalledWith('t-1', { codigo: 'bodega', nombre: 'Bodega' });

    await controller.actualizar(
      UBICACION,
      { activo: false } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.actualizar).toHaveBeenCalledWith('t-1', UBICACION, { activo: false });
  });

  it('delega stock, traslados y consultas por item', async () => {
    const { servicio, controller } = fabricar();
    servicio.listarStock.mockResolvedValue([]);
    servicio.stockDeItem.mockResolvedValue({ stock_total: 10 });
    servicio.ajustarStock.mockResolvedValue({ stock: 4 });
    servicio.trasladar.mockResolvedValue({ origen: {}, destino: {} });

    await controller.listarStock(UBICACION, undefined, empresario as never);
    expect(servicio.listarStock).toHaveBeenCalledWith('t-1', UBICACION);

    await controller.stockItem(ITEM, undefined, empresario as never);
    expect(servicio.stockDeItem).toHaveBeenCalledWith('t-1', ITEM);

    await controller.ajustarStock(
      UBICACION,
      ITEM,
      { stock: 4 } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.ajustarStock).toHaveBeenCalledWith('t-1', UBICACION, ITEM, 4);

    await controller.trasladar(
      { item_id: ITEM, origen_id: UBICACION, destino_id: 'otra', cantidad: 3 } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.trasladar).toHaveBeenCalledWith('t-1', {
      item_id: ITEM,
      origen_id: UBICACION,
      destino_id: 'otra',
      cantidad: 3,
    });
  });

  it('permite al admin operar sobre otra empresa', async () => {
    const { servicio, controller } = fabricar();
    servicio.listar.mockResolvedValue([]);
    await controller.listar('e-9', admin as never);
    expect(servicio.listar).toHaveBeenCalledWith('e-9');
  });
});
