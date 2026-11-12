import { describe, expect, it, vi } from 'vitest';
import { InternalController } from './internal.controller';

function crear() {
  const ofertas = {
    porIds: vi.fn().mockResolvedValue([]),
    contarActivasPorProducto: vi.fn().mockResolvedValue(0),
  };
  const tiendas = {
    porIds: vi.fn().mockResolvedValue([]),
    deEmpresa: vi.fn().mockResolvedValue([]),
    deVendedor: vi.fn().mockResolvedValue(null),
    encontrarPorId: vi.fn().mockResolvedValue({
      id: 't-1',
      empresa_id: 'e-1',
      nombre: 'Tienda',
      slug: 'tienda',
    }),
    asignarVendedor: vi.fn().mockResolvedValue(undefined),
    desasignarVendedor: vi.fn().mockResolvedValue(undefined),
  };
  const controller = new InternalController(ofertas as never, tiendas as never);
  return { controller, ofertas, tiendas };
}

describe('InternalController (servicio→servicio)', () => {
  it('lista ofertas por lote y filtra ids vacíos', async () => {
    const { controller, ofertas } = crear();
    await controller.porIds('a, b, ,c');
    expect(ofertas.porIds).toHaveBeenCalledWith(['a', 'b', 'c']);
    await controller.porIds();
    expect(ofertas.porIds).toHaveBeenLastCalledWith([]);
  });

  it('cuenta ofertas activas de un producto', async () => {
    const { controller, ofertas } = crear();
    ofertas.contarActivasPorProducto.mockResolvedValue(3);
    await expect(controller.activas('p-1')).resolves.toEqual({ total: 3 });
  });

  it('lista tiendas por lote, empresa y vendedor', async () => {
    const { controller, tiendas } = crear();
    await controller.tiendasPorIds('t-1,t-2');
    expect(tiendas.porIds).toHaveBeenCalledWith(['t-1', 't-2']);
    await controller.tiendasPorIds();
    expect(tiendas.porIds).toHaveBeenLastCalledWith([]);
    await controller.tiendasDeEmpresa('e-1');
    expect(tiendas.deEmpresa).toHaveBeenCalledWith('e-1');
    await controller.tiendaDeVendedor('v-1');
    expect(tiendas.deVendedor).toHaveBeenCalledWith('v-1');
  });

  it('devuelve la tienda o lanza NotFoundError', async () => {
    const { controller, tiendas } = crear();
    await expect(controller.tienda('t-1')).resolves.toMatchObject({ id: 't-1' });
    tiendas.encontrarPorId.mockResolvedValueOnce(null);
    await expect(controller.tienda('t-x')).rejects.toMatchObject({ codigo: 'NO_ENCONTRADO' });
  });

  it('asigna y desasigna vendedores', async () => {
    const { controller, tiendas } = crear();
    await expect(
      controller.asignarVendedor('t-1', { usuario_id: 'u-1', participacion: 25 }),
    ).resolves.toEqual({ ok: true });
    expect(tiendas.asignarVendedor).toHaveBeenCalledWith('t-1', 'e-1', 'u-1', 25);

    tiendas.encontrarPorId.mockResolvedValueOnce(null);
    await expect(
      controller.asignarVendedor('t-x', { usuario_id: 'u-1', participacion: 25 }),
    ).rejects.toMatchObject({ codigo: 'NO_ENCONTRADO' });

    await expect(
      controller.desasignarVendedor('t-1', { usuario_id: 'u-1' }),
    ).resolves.toEqual({ ok: true });
    expect(tiendas.desasignarVendedor).toHaveBeenCalledWith('t-1', 'u-1');
  });
});
