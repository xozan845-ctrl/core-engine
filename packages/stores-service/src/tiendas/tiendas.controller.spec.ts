import type { Mock } from 'vitest';
import 'reflect-metadata';
import { TiendasController, TiendasEmpresaController } from './tiendas.controller';
import type { TiendasService } from './tiendas.service';
import { NotFoundError, ROLES } from '@core/shared';

const tienda = { id: 't-1', vendedor_id: 'u-v', nombre: 'Mi tienda', descripcion: 'x' };
const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR, tenant_id: 'e-1', participacion_porcentaje: 25 };
const empresario = { user_id: 'u-e', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 'e-1' };

function fabricar(): { tiendas: Record<string, Mock>; controller: TiendasController } {
  const tiendas = { crear: vi.fn(), deVendedor: vi.fn(), deEmpresa: vi.fn() };
  const controller = new TiendasController(tiendas as unknown as TiendasService);
  return { tiendas, controller };
}

describe('TiendasController (api/v1/vendedores, R-U-10/11)', () => {
  it('debe crear la tienda con el vendedor y los datos del dto', async () => {
    const { tiendas, controller } = fabricar();
    tiendas.crear.mockResolvedValue(tienda);
    await expect(controller.crear({ nombre: 'Mi tienda', descripcion: 'x' } as never, vendedor as never)).resolves.toEqual(tienda);
    expect(tiendas.crear).toHaveBeenCalledWith('e-1', 'u-v', 25, 'Mi tienda', 'x');
  });

  it('debe asignar participacion completa a la tienda del empresario', async () => {
    const { tiendas, controller } = fabricar();
    const empresario = { ...vendedor, rol: ROLES.EMPRESARIO, user_id: 'u-e' };
    await controller.crear({ nombre: 'Tienda empresa' } as never, empresario as never);
    expect(tiendas.crear).toHaveBeenCalledWith('e-1', 'u-e', 100, 'Tienda empresa', undefined);
  });

  it('debe rechazar la creacion sin empresa activa', async () => {
    const { controller } = fabricar();
    await expect(controller.crear(
      { nombre: 'Mi tienda' } as never,
      { ...vendedor, tenant_id: undefined } as never,
    )).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe rechazar un vendedor sin participacion configurada', async () => {
    const { controller } = fabricar();
    await expect(controller.crear(
      { nombre: 'Mi tienda' } as never,
      { ...vendedor, participacion_porcentaje: undefined } as never,
    )).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe devolver la tienda del vendedor autenticado', async () => {
    const { tiendas, controller } = fabricar();
    tiendas.deVendedor.mockResolvedValue(tienda);
    await expect(controller.miTienda(vendedor as never)).resolves.toEqual(tienda);
    expect(tiendas.deVendedor).toHaveBeenCalledWith('u-v');
  });

  it('debe lanzar NotFoundError cuando el vendedor no tiene tienda', async () => {
    const { tiendas, controller } = fabricar();
    tiendas.deVendedor.mockResolvedValue(null);
    await expect(controller.miTienda(vendedor as never)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe exigir rol vendedor en crear y mi-tienda', () => {
    expect(Reflect.getMetadata('roles_requeridos', TiendasController.prototype.crear)).toEqual([ROLES.VENDEDOR, ROLES.EMPRESARIO]);
    expect(Reflect.getMetadata('roles_requeridos', TiendasController.prototype.miTienda)).toEqual([ROLES.VENDEDOR]);
  });
});

describe('TiendasEmpresaController (api/v1/empresarios/me/tiendas, ADR-25)', () => {
  function fabricarEmpresa(): { tiendas: Record<string, Mock>; controller: TiendasEmpresaController } {
    const tiendas = { deEmpresa: vi.fn() };
    const controller = new TiendasEmpresaController(tiendas as unknown as TiendasService);
    return { tiendas, controller };
  }

  it('debe listar las tiendas de la empresa activa', async () => {
    const { tiendas, controller } = fabricarEmpresa();
    tiendas.deEmpresa.mockResolvedValue([tienda]);
    await expect(controller.listar(empresario as never)).resolves.toEqual([tienda]);
    expect(tiendas.deEmpresa).toHaveBeenCalledWith('e-1');
  });

  it('debe rechazar al empresario sin empresa activa', async () => {
    const { tiendas, controller } = fabricarEmpresa();
    await expect(
      controller.listar({ ...empresario, tenant_id: undefined } as never),
    ).rejects.toBeInstanceOf(NotFoundError);
    expect(tiendas.deEmpresa).not.toHaveBeenCalled();
  });

  it('debe exigir rol empresario', () => {
    expect(Reflect.getMetadata('roles_requeridos', TiendasEmpresaController.prototype.listar)).toEqual([
      ROLES.EMPRESARIO,
    ]);
  });
});
