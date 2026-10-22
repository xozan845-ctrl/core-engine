import type { Mock } from 'vitest';
import 'reflect-metadata';
import { ProductosController } from './productos.controller';
import type { ProductosService } from './productos.service';
import { NotFoundError } from '@core/shared';

const producto = {
  empresa_id: 'e-1',
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
  function fabricar(): { productos: Record<string, Mock>; controller: ProductosController } {
    const productos = {
      listar: vi.fn(),
      encontrarPorId: vi.fn(),
      crear: vi.fn(),
      actualizar: vi.fn(),
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
    await expect(controller.crear(dto as never, { user_id: 'u-1', email: 'e@core.local', rol: 'empresario', tenant_id: 'e-1' } as never)).resolves.toEqual(producto);
    expect(productos.crear).toHaveBeenCalledWith({ ...dto, empresa_id: 'e-1', precio_base: '1500.00' });
  });

  it('debe crear el producto usando la empresa indicada por un super_admin', async () => {
    const { productos, controller } = fabricar();
    productos.crear.mockResolvedValue(producto);
    const dto = {
      empresa_id: 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a22',
      sku: 'SKU-02',
      nombre: 'Mouse',
      precio_base: '500.00',
      stock: 4,
    };
    await expect(
      controller.crear(dto as never, {
        user_id: 'u-sa',
        email: 'sa@core.local',
        rol: 'super_admin',
      } as never),
    ).resolves.toEqual(producto);
    expect(productos.crear).toHaveBeenCalledWith({ ...dto, precio_base: '500.00' });
  });

  it('debe lanzar NotFoundError cuando un super_admin crea sin indicar empresa', async () => {
    const { productos, controller } = fabricar();
    await expect(
      controller.crear({ sku: 'SKU-02', nombre: 'Mouse', precio_base: '500.00', stock: 4 } as never, {
        user_id: 'u-sa',
        email: 'sa@core.local',
        rol: 'super_admin',
      } as never),
    ).rejects.toBeInstanceOf(NotFoundError);
    expect(productos.crear).not.toHaveBeenCalled();
  });

  it('debe actualizar pasando id y dto de cambio al servicio', async () => {
    const { productos, controller } = fabricar();
    productos.actualizar.mockResolvedValue(producto);
    const dto = { stock: 5, motivo: 'merma' };
    await expect(controller.actualizar(producto.id, dto as never, { user_id: 'u-1', email: 'e@core.local', rol: 'empresario', tenant_id: 'e-1' } as never)).resolves.toEqual(producto);
    expect(productos.actualizar).toHaveBeenCalledWith(producto.id, dto, 'e-1');
  });

  it('debe lanzar NotFoundError cuando el empresario no tiene empresa activa al crear', async () => {
    const { productos, controller } = fabricar();
    await expect(
      controller.crear({ sku: 'SKU-01', nombre: 'Teclado', precio_base: '1500.00', stock: 10 } as never, {
        user_id: 'u-1',
        email: 'e@core.local',
        rol: 'empresario',
      } as never),
    ).rejects.toBeInstanceOf(NotFoundError);
    expect(productos.crear).not.toHaveBeenCalled();
  });

  it('debe actualizar sin filtro de empresa cuando el rol no es empresario', async () => {
    const { productos, controller } = fabricar();
    productos.actualizar.mockResolvedValue(producto);
    const dto = { stock: 5 };
    await controller.actualizar(producto.id, dto as never, {
      user_id: 'u-admin',
      email: 'admin@core.local',
      rol: 'admin',
    } as never);
    expect(productos.actualizar).toHaveBeenCalledWith(producto.id, dto, undefined);
  });

  it('debe lanzar NotFoundError cuando el empresario no tiene empresa activa al actualizar', async () => {
    const { productos, controller } = fabricar();
    await expect(
      controller.actualizar(producto.id, { stock: 5 } as never, {
        user_id: 'u-1',
        email: 'e@core.local',
        rol: 'empresario',
      } as never),
    ).rejects.toBeInstanceOf(NotFoundError);
    expect(productos.actualizar).not.toHaveBeenCalled();
  });

  it('debe reservar la escritura al empresario y a los operadores globales', () => {
    const esperado = ['super_admin', 'admin', 'empresario'];
    expect(Reflect.getMetadata('roles_requeridos', ProductosController.prototype.crear)).toEqual(esperado);
    expect(Reflect.getMetadata('roles_requeridos', ProductosController.prototype.actualizar)).toEqual(esperado);
  });
});
