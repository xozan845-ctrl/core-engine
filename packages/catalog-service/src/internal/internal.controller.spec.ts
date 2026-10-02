import 'reflect-metadata';
import { InternalController } from './internal.controller';
import type { ProductosService } from '../productos/productos.service';
import type { InventarioService } from '../inventario/inventario.service';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';

const producto = {
  id: 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11',
  sku: 'SKU-01',
  nombre: 'Teclado',
  descripcion: 'Teclado mecanico',
  categoria: 'perifericos',
  precio_base: '1500.00',
  stock: 10,
  estado: 'disponible',
  creado_en: '2026-01-01T00:00:00.000Z',
};

function fabricar(): {
  productos: Record<string, jest.Mock>;
  inventario: Record<string, jest.Mock>;
  controller: InternalController;
} {
  const productos = {
    lotePorSkus: jest.fn(),
    encontrarPorSku: jest.fn(),
    encontrarPorId: jest.fn(),
    resumenInventario: jest.fn(),
  };
  const inventario = { listar: jest.fn() };
  const controller = new InternalController(
    productos as unknown as ProductosService,
    inventario as unknown as InventarioService,
  );
  return { productos, inventario, controller };
}

describe('InternalController (internal, R-U-10/11)', () => {
  it('debe dividir la query de skus en lista limpia y consultar el lote', async () => {
    const { productos, controller } = fabricar();
    productos.lotePorSkus.mockResolvedValue([{ sku: 'SKU-01', stock: 10, estado: 'disponible' }]);
    await controller.lote('SKU-01, , SKU-02');
    expect(productos.lotePorSkus).toHaveBeenCalledWith(['SKU-01', 'SKU-02']);
  });

  it('debe consultar el lote vacio cuando no llegan skus', async () => {
    const { productos, controller } = fabricar();
    productos.lotePorSkus.mockResolvedValue([]);
    await controller.lote(undefined);
    expect(productos.lotePorSkus).toHaveBeenCalledWith([]);
  });

  it('debe devolver el producto por sku cuando existe y lanzar NotFoundError cuando no', async () => {
    const { productos, controller } = fabricar();
    productos.encontrarPorSku.mockResolvedValue(producto);
    await expect(controller.porSku('SKU-01')).resolves.toEqual(producto);
    productos.encontrarPorSku.mockResolvedValue(null);
    await expect(controller.porSku('SKU-X')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe devolver el producto por id cuando existe y lanzar NotFoundError cuando no', async () => {
    const { productos, controller } = fabricar();
    productos.encontrarPorId.mockResolvedValue(producto);
    await expect(controller.porId(producto.id)).resolves.toEqual(producto);
    productos.encontrarPorId.mockResolvedValue(null);
    await expect(controller.porId(producto.id)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe devolver el resumen de inventario y las lineas de la bodega', async () => {
    const { productos, inventario, controller } = fabricar();
    const resumen = { productos: 1, agotados: 0, stock_total: 10, valor_inventario_cents: 1500000 };
    productos.resumenInventario.mockResolvedValue(resumen);
    inventario.listar.mockResolvedValue([]);
    await expect(controller.resumen()).resolves.toEqual(resumen);
    await expect(controller.lineas()).resolves.toEqual([]);
  });

  it('debe proteger todas las rutas con la clave interna', () => {
    const guards = Reflect.getMetadata('__guards__', InternalController) as unknown[];
    expect(guards).toBeDefined();
    expect(guards.some((g) => g === ClaveInternaGuard)).toBe(true);
  });
});