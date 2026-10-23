import type { Mock } from 'vitest';
import 'reflect-metadata';
import { OfertasController } from './ofertas.controller';
import type { OfertasService } from './ofertas.service';
import type { TiendasService } from '../tiendas/tiendas.service';
import { NotFoundError, ROLES } from '@core/shared';

const oferta = { id: 'of-1', producto_id: 'p-1', vendedor_id: 'u-v', margen: 15, precio_final: '1150.00' };
const tienda = { id: 't-1', vendedor_id: 'u-v', nombre: 'Mi tienda', slug: 'mi-tienda' };
const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR };

function respuesta(): { status: Mock; setHeader: Mock } {
  return {
    status: vi.fn().mockReturnThis(),
    setHeader: vi.fn(),
  };
}

function fabricar(): {
  tiendas: Record<string, Mock>;
  ofertas: Record<string, Mock>;
  controller: OfertasController;
} {
  const tiendas = { encontrarPorId: vi.fn(), deVendedor: vi.fn(), resolver: vi.fn() };
  const ofertas = {
    publicar: vi.fn(),
    cambiarMargen: vi.fn(),
    deVendedor: vi.fn(),
    deTienda: vi.fn(),
  };
  const controller = new OfertasController(
    tiendas as unknown as TiendasService,
    ofertas as unknown as OfertasService,
  );
  return { tiendas, ofertas, controller };
}

describe('OfertasController (api/v1, RN-01, R-U-10/11)', () => {
  it('debe publicar la oferta con vendedor, producto y margen', async () => {
    const { tiendas, ofertas, controller } = fabricar();
    tiendas.encontrarPorId.mockResolvedValue(tienda);
    ofertas.publicar.mockResolvedValue(oferta);
    const dto = { tienda_id: '11111111-2222-3333-4444-555555555555', producto_id: 'p-1', margen: 15 };
    await expect(controller.publicar(dto as never, vendedor as never)).resolves.toEqual(oferta);
    expect(tiendas.encontrarPorId).toHaveBeenCalledWith(dto.tienda_id);
    expect(ofertas.publicar).toHaveBeenCalledWith('u-v', 't-1', 'p-1', 15);
  });

  it('debe publicar usando una tienda resuelta por slug', async () => {
    const { tiendas, ofertas, controller } = fabricar();
    tiendas.encontrarPorId.mockResolvedValue(tienda);
    ofertas.publicar.mockResolvedValue(oferta);
    const dto = { tienda_id: 'mi-tienda', producto_id: 'p-1', margen: 15 };

    await expect(controller.publicar(dto as never, vendedor as never)).resolves.toEqual(oferta);
    expect(tiendas.encontrarPorId).toHaveBeenCalledWith('mi-tienda');
    expect(ofertas.publicar).toHaveBeenCalledWith('u-v', 't-1', 'p-1', 15);
  });

  it('debe lanzar NotFoundError si la tienda indicada no existe', async () => {
    const { tiendas, ofertas, controller } = fabricar();
    tiendas.encontrarPorId.mockResolvedValue(null);
    await expect(
      controller.publicar({ tienda_id: 'no-existe', producto_id: 'p-1', margen: 15 } as never, vendedor as never),
    ).rejects.toBeInstanceOf(NotFoundError);
    expect(ofertas.publicar).not.toHaveBeenCalled();
  });

  it('debe usar la tienda del vendedor cuando no se informa tienda_id', async () => {
    const { tiendas, ofertas, controller } = fabricar();
    tiendas.deVendedor.mockResolvedValue(tienda);
    ofertas.publicar.mockResolvedValue(oferta);
    const dto = { producto_id: 'p-1', margen: 15 };
    await expect(controller.publicar(dto as never, vendedor as never)).resolves.toEqual(oferta);
    expect(tiendas.deVendedor).toHaveBeenCalledWith('u-v');
    expect(ofertas.publicar).toHaveBeenCalledWith('u-v', 't-1', 'p-1', 15);
  });

  it('debe lanzar NotFoundError si no hay tienda y no se informa tienda_id', async () => {
    const { tiendas, ofertas, controller } = fabricar();
    tiendas.deVendedor.mockResolvedValue(null);
    await expect(
      controller.publicar({ producto_id: 'p-1', margen: 15 } as never, vendedor as never),
    ).rejects.toBeInstanceOf(NotFoundError);
    expect(ofertas.publicar).not.toHaveBeenCalled();
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

  it('debe devolver la tienda publica con sus ofertas resolviendo por UUID o slug', async () => {
    const { tiendas, ofertas, controller } = fabricar();
    tiendas.resolver.mockResolvedValue({ tienda });
    ofertas.deTienda.mockResolvedValue([oferta]);

    await expect(controller.tiendaPublica('mi-tienda', respuesta() as never)).resolves.toEqual({
      tienda,
      ofertas: [oferta],
    });
    expect(tiendas.resolver).toHaveBeenCalledWith('mi-tienda');
    expect(ofertas.deTienda).toHaveBeenCalledWith('t-1');
  });

  it('debe responder 301 cuando el slug público es anterior', async () => {
    const { tiendas, ofertas, controller } = fabricar();
    tiendas.resolver.mockResolvedValue({
      tienda,
      redirigirA: '/api/v1/tiendas/mi-tienda-nueva',
    });
    ofertas.deTienda.mockResolvedValue([]);
    const res = respuesta();

    await controller.tiendaPublica('mi-tienda-vieja', res as never);

    expect(res.status).toHaveBeenCalledWith(301);
    expect(res.setHeader).toHaveBeenCalledWith('Location', '/api/v1/tiendas/mi-tienda-nueva');
  });

  it('debe lanzar NotFoundError cuando la tienda publica no existe', async () => {
    const { tiendas, controller } = fabricar();
    tiendas.resolver.mockResolvedValue(null);
    await expect(controller.tiendaPublica('t-x', respuesta() as never)).rejects.toBeInstanceOf(
      NotFoundError,
    );
  });

  it('debe exigir rol vendedor en publicar y cambiar margen', () => {
    expect(Reflect.getMetadata('roles_requeridos', OfertasController.prototype.publicar)).toEqual([ROLES.VENDEDOR, ROLES.EMPRESARIO]);
    expect(Reflect.getMetadata('roles_requeridos', OfertasController.prototype.cambiarMargen)).toEqual([ROLES.VENDEDOR, ROLES.EMPRESARIO]);
  });
});
