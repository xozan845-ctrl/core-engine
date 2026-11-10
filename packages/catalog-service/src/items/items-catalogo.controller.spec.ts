import type { Mock } from 'vitest';
import 'reflect-metadata';
import {
  ItemsCatalogoController,
  ActualizarItemCatalogoRequestDto,
} from './items-catalogo.controller';
import type { ItemsService } from './items.service';
import type { ProductosService } from '../productos/productos.service';
import type { ServiciosService } from '../servicios/servicios.service';
import { NotFoundError, ROLES } from '@core/shared';

const EMPRESA = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';
const ITEM_ID = '11111111-2222-3333-4444-555555555555';

const itemProducto = { item_id: ITEM_ID, empresa_id: EMPRESA, tipo: 'producto' as const };
const itemServicio = { item_id: ITEM_ID, empresa_id: EMPRESA, tipo: 'servicio' as const };
const itemAlquiler = { item_id: ITEM_ID, empresa_id: EMPRESA, tipo: 'alquiler' as const };

const usuarioAdmin = { rol: ROLES.ADMIN, tenant_id: null, user_id: 'u-1' } as never;
const usuarioEmpresario = { rol: ROLES.EMPRESARIO, tenant_id: EMPRESA, user_id: 'u-2' } as never;

function fabricar(): {
  items: Record<string, Mock>;
  productos: Record<string, Mock>;
  servicios: Record<string, Mock>;
  controller: ItemsCatalogoController;
} {
  const items = { encontrarPorId: vi.fn(), eliminar: vi.fn() };
  const productos = {
    actualizar: vi.fn(),
    eliminar: vi.fn(),
    crearVariante: vi.fn(),
    actualizarVariante: vi.fn(),
    eliminarVariante: vi.fn(),
    agregarImagen: vi.fn(),
    eliminarImagen: vi.fn(),
  };
  const servicios = { actualizar: vi.fn(), eliminar: vi.fn() };
  const controller = new ItemsCatalogoController(
    items as unknown as ItemsService,
    productos as unknown as ProductosService,
    servicios as unknown as ServiciosService,
  );
  return { items, productos, servicios, controller };
}

describe('ItemsCatalogoController (catálogo unificado)', () => {
  it('edita un producto mapeando precio y estado', async () => {
    const { items, productos, controller } = fabricar();
    items.encontrarPorId.mockResolvedValue(itemProducto);
    productos.actualizar.mockResolvedValue({ id: ITEM_ID });
    const dto: ActualizarItemCatalogoRequestDto = {
      precio: '1200.00',
      estado: 'inactivo',
      stock: 4,
    };

    await expect(controller.actualizar(ITEM_ID, dto, usuarioAdmin)).resolves.toEqual({ id: ITEM_ID });
    expect(productos.actualizar).toHaveBeenCalledWith(
      ITEM_ID,
      expect.objectContaining({
        precio_base: '1200.00',
        estado_operativo: 'inactivo',
        stock: 4,
      }),
      EMPRESA,
    );
  });

  it('edita un servicio mapeando precio_base a precio', async () => {
    const { items, servicios, controller } = fabricar();
    items.encontrarPorId.mockResolvedValue(itemServicio);
    servicios.actualizar.mockResolvedValue({ id: ITEM_ID });

    await expect(
      controller.actualizar(ITEM_ID, { precio_base: '900.00', estado_operativo: 'inactivo' }, usuarioAdmin),
    ).resolves.toEqual({ id: ITEM_ID });
    expect(servicios.actualizar).toHaveBeenCalledWith(
      ITEM_ID,
      expect.objectContaining({ precio: '900.00', estado: 'inactivo' }),
      EMPRESA,
    );
  });

  it('da de baja un alquiler y rechaza editarlo', async () => {
    const { items, controller } = fabricar();
    items.encontrarPorId.mockResolvedValue(itemAlquiler);

    await expect(
      controller.actualizar(ITEM_ID, { nombre: 'Otro' }, usuarioAdmin),
    ).rejects.toMatchObject({ codigo: 'ITEM_NO_EDITABLE' });
    await expect(controller.actualizar(ITEM_ID, { estado: 'inactivo' }, usuarioAdmin)).resolves.toBeUndefined();
    expect(items.eliminar).toHaveBeenCalledWith(ITEM_ID, EMPRESA);
  });

  it('elimina productos, servicios y alquileres según el tipo', async () => {
    const { items, productos, servicios, controller } = fabricar();
    items.encontrarPorId
      .mockResolvedValueOnce(itemProducto)
      .mockResolvedValueOnce(itemServicio)
      .mockResolvedValueOnce(itemAlquiler);

    await controller.eliminar(ITEM_ID, usuarioAdmin);
    await controller.eliminar(ITEM_ID, usuarioAdmin);
    await controller.eliminar(ITEM_ID, usuarioAdmin);

    expect(productos.eliminar).toHaveBeenCalledWith(ITEM_ID, EMPRESA);
    expect(servicios.eliminar).toHaveBeenCalledWith(ITEM_ID, EMPRESA);
    expect(items.eliminar).toHaveBeenCalledWith(ITEM_ID, EMPRESA);
  });

  it('delega variantes e imágenes de un producto', async () => {
    const { items, productos, controller } = fabricar();
    items.encontrarPorId.mockResolvedValue(itemProducto);
    productos.crearVariante.mockResolvedValue({ id: 'v-1' });
    productos.actualizarVariante.mockResolvedValue({ id: 'v-1' });
    productos.agregarImagen.mockResolvedValue({ id: 'i-1' });

    await controller.crearVariante(ITEM_ID, { sku: 'SKU-01-A', precio_base: '10.00', stock: 1 }, usuarioAdmin);
    await controller.actualizarVariante(ITEM_ID, 'v-1', { stock: 2 }, usuarioAdmin);
    await controller.eliminarVariante(ITEM_ID, 'v-1', usuarioAdmin);
    await controller.agregarImagen(ITEM_ID, { objeto_id: 'obj-1' }, usuarioAdmin);
    await controller.eliminarImagen(ITEM_ID, 'img-1', usuarioAdmin);

    expect(productos.crearVariante).toHaveBeenCalledWith(
      ITEM_ID,
      expect.objectContaining({ sku: 'SKU-01-A' }),
      EMPRESA,
    );
    expect(productos.actualizarVariante).toHaveBeenCalledWith(
      ITEM_ID,
      'v-1',
      expect.objectContaining({ stock: 2 }),
      EMPRESA,
    );
    expect(productos.eliminarVariante).toHaveBeenCalledWith(ITEM_ID, 'v-1', EMPRESA);
    expect(productos.agregarImagen).toHaveBeenCalledWith(ITEM_ID, { objeto_id: 'obj-1' }, EMPRESA);
    expect(productos.eliminarImagen).toHaveBeenCalledWith(ITEM_ID, 'img-1', EMPRESA);
  });

  it('el empresario solo opera el catálogo de su empresa', async () => {
    const { items, controller } = fabricar();
    items.encontrarPorId.mockResolvedValue(itemProducto);

    await expect(controller.eliminar(ITEM_ID, usuarioEmpresario)).resolves.toBeUndefined();
    expect(items.encontrarPorId).toHaveBeenCalledWith(ITEM_ID, EMPRESA);

    items.encontrarPorId.mockResolvedValue(null);
    await expect(controller.eliminar(ITEM_ID, usuarioEmpresario)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('exige empresa activa al empresario y falla si el item no existe', async () => {
    const { controller } = fabricar();
    await expect(
      controller.eliminar(ITEM_ID, { rol: ROLES.EMPRESARIO, tenant_id: null } as never),
    ).rejects.toBeInstanceOf(NotFoundError);

    const { items, controller: otro } = fabricar();
    items.encontrarPorId.mockResolvedValue(null);
    await expect(otro.eliminar(ITEM_ID, usuarioAdmin)).rejects.toBeInstanceOf(NotFoundError);
  });
});
