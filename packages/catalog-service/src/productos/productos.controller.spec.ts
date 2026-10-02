import 'reflect-metadata';
import { ProductosController } from './productos.controller';
import type { ProductosService } from './productos.service';
import { NotFoundError } from '@core/shared';

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

describe('ProductosController (api/v1/catalog, R-U-10/11)', () => {
  function fabricar(): { productos: Record<string, jest.Mock>; controller: ProductosController } {
    const productos = {
      listar: jest.fn(),
      encontrarPorId: jest.fn(),
      crear: jest.fn(),
      actualizar: jest.fn(),
    };
    const controller = new ProductosController(productos as unknown as ProductosService);
    return { productos, controller };
  }

  it('debe listar reenviando los filtros de la query al servicio cuando llega una query', async () => {
    const { productos, controller } = fabricar();
    productos.listar.mockResolvedValue({ items: [producto], total: 1, pagina: 1, limite: 20, paginas: 1 });
    const query = { q: 'teclado', estado: 'disponible', pagina: '2', limite: '10' };
    const pagina = await controller.listar(query as never);
    expect(productos.listar).toHaveBeenCalledWith(query);
    expect(pagina.items).toEqual([producto]);
  });

  it('debe lanzar la pagina del servicio cuando no llegan filtros', async () => {
    const { productos, controller } = fabricar();
    productos.listar.mockResolvedValue({ items: [], total: 0, pagina: 1, limite: 20, paginas: 1 });
    await expect(controller.listar({} as never)).resolves.toMatchObject({ total: 0, paginas: 1 });
  });

  it('debe devolver el detalle cuando el producto existe', async () => {
    const { productos, controller } = fabricar();
    productos.encontrarPorId.mockResolvedValue(producto);
    await expect(controller.detalle(producto.id)).resolves.toEqual(producto);
    expect(productos.encontrarPorId).toHaveBeenCalledWith(producto.id);
  });

  it('debe lanzar NotFoundError cuando el detalle no existe', async () => {
    const { productos, controller } = fabricar();
    productos.encontrarPorId.mockResolvedValue(null);
    await expect(controller.detalle(producto.id)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe crear el producto pasando el dto completo al servicio', async () => {
    const { productos, controller } = fabricar();
    productos.crear.mockResolvedValue(producto);
    const dto = { sku: 'SKU-01', nombre: 'Teclado', precio_base: '1500.00', stock: 10 };
    await expect(controller.crear(dto as never, { user_id: 'u-1', email: 'admin@core.local', rol: 'admin' } as never)).resolves.toEqual(producto);
    expect(productos.crear).toHaveBeenCalledWith({ ...dto, precio_base: '1500.00' });
  });

  it('debe actualizar pasando id y dto de cambio al servicio', async () => {
    const { productos, controller } = fabricar();
    productos.actualizar.mockResolvedValue(producto);
    const dto = { stock: 5, motivo: 'merma' };
    await expect(controller.actualizar(producto.id, dto as never)).resolves.toEqual(producto);
    expect(productos.actualizar).toHaveBeenCalledWith(producto.id, dto);
  });

  it('debe exigir el rol admin en crear y actualizar', () => {
    expect(Reflect.getMetadata('roles_requeridos', ProductosController.prototype.crear)).toEqual(['admin']);
    expect(Reflect.getMetadata('roles_requeridos', ProductosController.prototype.actualizar)).toEqual(['admin']);
  });
});