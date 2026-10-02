import 'reflect-metadata';
import { OfertasController } from './ofertas.controller';
import type { OfertasService } from './ofertas.service';
import type { TiendasService } from '../tiendas/tiendas.service';
import { NotFoundError, ROLES } from '@core/shared';

const oferta = { id: 'of-1', producto_id: 'p-1', vendedor_id: 'u-v', margen: 15, precio_final: '1150.00' };
const tienda = { id: 't-1', vendedor_id: 'u-v', nombre: 'Mi tienda' };
const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR };

function fabricar(): {
  tiendas: Record<string, jest.Mock>;
  ofertas: Record<string, jest.Mock>;
  controller: OfertasController;
} {
  const tiendas = { encontrarPorId: jest.fn() };
  const ofertas = {
    publicar: jest.fn(),
    cambiarMargen: jest.fn(),
    deVendedor: jest.fn(),
    deTienda: jest.fn(),
  };
  const controller = new OfertasController(
    tiendas as unknown as TiendasService,
    ofertas as unknown as OfertasService,
  );
  return { tiendas, ofertas, controller };
}

describe('OfertasController (api/v1, RN-01, R-U-10/11)', () => {
  it('debe publicar la oferta con vendedor, producto y margen', async () => {
    const { ofertas, controller } = fabricar();
    ofertas.publicar.mockResolvedValue(oferta);
    await expect(controller.publicar({ producto_id: 'p-1', margen: 15 } as never, vendedor as never)).resolves.toEqual(oferta);
    expect(ofertas.publicar).toHaveBeenCalledWith('u-v', 'p-1', 15);
  });

  it('debe cambiar el margen de una oferta del vendedor autenticado', async () => {
    const { ofertas, controller } = fabricar();
    ofertas.cambiarMargen.mockResolvedValue(oferta);
    await controller.cambiarMargen('of-1', { margen: 20 } as never, vendedor as never);
    expect(ofertas.cambiarMargen).toHaveBeenCalledWith('of-1', 'u-v', 20);
  });

  it('debe listar las ofertas del vendedor reenviando la query', async () => {
    const { ofertas, controller } = fabricar();
    const pagina = { items: [oferta], total: 1, pagina: 1, limite: 20, paginas: 1 };
    ofertas.deVendedor.mockResolvedValue(pagina);
    const query = { pagina: '1', limite: '20' };
    await expect(controller.misOfertas(query, vendedor as never)).resolves.toEqual(pagina);
    expect(ofertas.deVendedor).toHaveBeenCalledWith('u-v', query);
  });

  it('debe devolver la tienda publica con sus ofertas', async () => {
    const { tiendas, ofertas, controller } = fabricar();
    tiendas.encontrarPorId.mockResolvedValue(tienda);
    ofertas.deTienda.mockResolvedValue([oferta]);
    await expect(controller.tiendaPublica('t-1')).resolves.toEqual({ tienda, ofertas: [oferta] });
    expect(ofertas.deTienda).toHaveBeenCalledWith('t-1');
  });

  it('debe lanzar NotFoundError cuando la tienda publica no existe', async () => {
    const { tiendas, controller } = fabricar();
    tiendas.encontrarPorId.mockResolvedValue(null);
    await expect(controller.tiendaPublica('t-x')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe exigir rol vendedor en publicar y cambiar margen', () => {
    expect(Reflect.getMetadata('roles_requeridos', OfertasController.prototype.publicar)).toEqual([ROLES.VENDEDOR]);
    expect(Reflect.getMetadata('roles_requeridos', OfertasController.prototype.cambiarMargen)).toEqual([ROLES.VENDEDOR]);
  });
});