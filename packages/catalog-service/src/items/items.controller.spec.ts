import 'reflect-metadata';
import { ItemsController } from './items.controller';
import type { ItemsService } from './items.service';
import { DomainError, NotFoundError, ROLES } from '@core/shared';

const EMPRESA = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';
const ITEM = '11111111-2222-3333-4444-555555555555';

function crear() {
  const items = {
    listar: vi.fn().mockResolvedValue({ items: [], total: 0 }),
    encontrarPorId: vi.fn(),
    crearAlquiler: vi.fn(),
    guardarEsquema: vi.fn(),
    esquemas: vi.fn().mockResolvedValue([]),
  };
  return {
    items,
    controller: new ItemsController(items as unknown as ItemsService),
  };
}

const empresario = { rol: ROLES.EMPRESARIO, tenant_id: EMPRESA } as never;
const admin = { rol: ROLES.ADMIN, tenant_id: null } as never;

describe('ItemsController (fase 2)', () => {
  it('lista y consulta items dentro del tenant', async () => {
    const { controller, items } = crear();
    items.encontrarPorId.mockResolvedValue({ item_id: ITEM, empresa_id: EMPRESA });

    await controller.listar({ tipo: 'servicio' }, empresario);
    expect(items.listar).toHaveBeenCalledWith({ tipo: 'servicio', empresa_id: EMPRESA });
    await expect(controller.detalle(ITEM, undefined, empresario)).resolves.toMatchObject({ item_id: ITEM });
  });

  it('rechaza consultar otra empresa y items inexistentes', async () => {
    const { controller, items } = crear();
    await expect(controller.listar({ empresa_id: 'otra' }, empresario)).rejects.toBeInstanceOf(DomainError);
    items.encontrarPorId.mockResolvedValue(null);
    await expect(controller.detalle(ITEM, EMPRESA, empresario)).rejects.toBeInstanceOf(NotFoundError);
    await expect(controller.listar({}, admin)).rejects.toMatchObject({ codigo: 'EMPRESA_REQUERIDA' });
    await expect(controller.listar({}, { rol: ROLES.EMPRESARIO, tenant_id: null } as never))
      .rejects.toMatchObject({ codigo: 'EMPRESA_REQUERIDA' });
    items.encontrarPorId.mockResolvedValue({ item_id: ITEM, empresa_id: EMPRESA });
    await expect(controller.detalle(ITEM, EMPRESA, admin)).resolves.toMatchObject({ item_id: ITEM });
  });

  it('crea alquileres y usa empresa explícita para admin', async () => {
    const { controller, items } = crear();
    items.crearAlquiler.mockResolvedValue({ item_id: ITEM, tipo: 'alquiler' });
    await expect(controller.crearAlquiler({ empresa_id: EMPRESA, codigo: 'ALQ-01', nombre: 'Sala', precio: '10.00' }, admin))
      .resolves.toMatchObject({ tipo: 'alquiler' });
    expect(items.crearAlquiler).toHaveBeenCalledWith(
      expect.objectContaining({ codigo: 'ALQ-01' }),
      EMPRESA,
    );
    await expect(controller.crearAlquiler({ codigo: 'ALQ-01', nombre: 'Sala', precio: '10.00' }, admin))
      .rejects.toMatchObject({ codigo: 'EMPRESA_REQUERIDA' });
    await expect(controller.crearAlquiler({ codigo: 'ALQ-01', nombre: 'Sala', precio: '10.00' }, { rol: ROLES.EMPRESARIO, tenant_id: null } as never))
      .rejects.toMatchObject({ codigo: 'EMPRESA_REQUERIDA' });
    await expect(controller.crearAlquiler({ codigo: 'ALQ-01', nombre: 'Sala', precio: '10.00' }, empresario))
      .resolves.toMatchObject({ tipo: 'alquiler' });
    expect(items.crearAlquiler).toHaveBeenLastCalledWith(
      expect.objectContaining({ codigo: 'ALQ-01' }),
      EMPRESA,
    );
  });

  it('registra y lista esquemas de atributos', async () => {
    const { controller, items } = crear();
    const esquema = { type: 'object' };
    await expect(controller.guardarEsquema({ tipo: 'producto', categoria: 'farmacia', esquema }, empresario))
      .resolves.toEqual({ ok: true });
    expect(items.guardarEsquema).toHaveBeenCalledWith(EMPRESA, 'producto', 'farmacia', esquema);
    await expect(controller.esquemas(empresario)).resolves.toEqual([]);
  });
});
