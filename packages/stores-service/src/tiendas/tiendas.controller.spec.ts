import 'reflect-metadata';
import { TiendasController } from './tiendas.controller';
import type { TiendasService } from './tiendas.service';
import { NotFoundError, ROLES } from '@core/shared';

const tienda = { id: 't-1', vendedor_id: 'u-v', nombre: 'Mi tienda', descripcion: 'x' };
const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR };

function fabricar(): { tiendas: Record<string, jest.Mock>; controller: TiendasController } {
  const tiendas = { crear: jest.fn(), deVendedor: jest.fn() };
  const controller = new TiendasController(tiendas as unknown as TiendasService);
  return { tiendas, controller };
}

describe('TiendasController (api/v1/vendedores, R-U-10/11)', () => {
  it('debe crear la tienda con el vendedor y los datos del dto', async () => {
    const { tiendas, controller } = fabricar();
    tiendas.crear.mockResolvedValue(tienda);
    await expect(controller.crear({ nombre: 'Mi tienda', descripcion: 'x' } as never, vendedor as never)).resolves.toEqual(tienda);
    expect(tiendas.crear).toHaveBeenCalledWith('u-v', 'Mi tienda', 'x');
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
    expect(Reflect.getMetadata('roles_requeridos', TiendasController.prototype.crear)).toEqual([ROLES.VENDEDOR]);
    expect(Reflect.getMetadata('roles_requeridos', TiendasController.prototype.miTienda)).toEqual([ROLES.VENDEDOR]);
  });
});