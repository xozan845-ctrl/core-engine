import 'reflect-metadata';
import { CarritoController } from './carrito.controller';
import type { CarritoService } from './carrito.service';
import { ROLES } from '@core/shared';

const carrito = { comprador_id: 'u-c', items: [], total_cents: 0 };
const comprador = { user_id: 'u-c', email: 'c@core.local', rol: ROLES.COMPRADOR };

function fabricar(): { carritos: Record<string, jest.Mock>; controller: CarritoController } {
  const carritos = {
    obtener: jest.fn(),
    agregar: jest.fn(),
    actualizarCantidad: jest.fn(),
    quitar: jest.fn(),
    vaciar: jest.fn(),
  };
  const controller = new CarritoController(carritos as unknown as CarritoService);
  return { carritos, controller };
}

describe('CarritoController (api/v1/carrito, RN-05, R-U-10/11)', () => {
  it('debe ver el carrito del comprador autenticado', async () => {
    const { carritos, controller } = fabricar();
    carritos.obtener.mockResolvedValue(carrito);
    await expect(controller.ver(comprador as never)).resolves.toEqual(carrito);
    expect(carritos.obtener).toHaveBeenCalledWith('u-c');
  });

  it('debe agregar una oferta con su cantidad al carrito', async () => {
    const { carritos, controller } = fabricar();
    carritos.agregar.mockResolvedValue(carrito);
    await controller.agregar({ oferta_id: 'of-1', cantidad: 3 } as never, comprador as never);
    expect(carritos.agregar).toHaveBeenCalledWith('u-c', 'of-1', 3);
  });

  it('debe actualizar la cantidad de un item del carrito', async () => {
    const { carritos, controller } = fabricar();
    carritos.actualizarCantidad.mockResolvedValue(carrito);
    await controller.actualizar('of-1', { cantidad: 5 } as never, comprador as never);
    expect(carritos.actualizarCantidad).toHaveBeenCalledWith('u-c', 'of-1', 5);
  });

  it('debe quitar un item del carrito', async () => {
    const { carritos, controller } = fabricar();
    carritos.quitar.mockResolvedValue(carrito);
    await controller.quitar('of-1', comprador as never);
    expect(carritos.quitar).toHaveBeenCalledWith('u-c', 'of-1');
  });

  it('debe vaciar el carrito', async () => {
    const { carritos, controller } = fabricar();
    carritos.vaciar.mockResolvedValue(carrito);
    await controller.vaciar(comprador as never);
    expect(carritos.vaciar).toHaveBeenCalledWith('u-c');
  });

  it('debe exigir rol comprador en todas las rutas', () => {
    expect(Reflect.getMetadata('roles_requeridos', CarritoController.prototype.ver)).toEqual([ROLES.COMPRADOR]);
    expect(Reflect.getMetadata('roles_requeridos', CarritoController.prototype.agregar)).toEqual([ROLES.COMPRADOR]);
  });
});