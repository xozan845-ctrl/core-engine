import type { Mock } from 'vitest';
import 'reflect-metadata';
import { InternalController } from './internal.controller';
import type { InventarioService } from '../inventario/inventario.service';
import type { ItemsService } from '../items/items.service';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';

const item = {
  item_id: '11111111-2222-3333-4444-555555555555',
  tipo: 'producto',
  codigo: 'SKU-01',
  nombre: 'Teclado',
};

function fabricar(): {
  inventario: Record<string, Mock>;
  items: Record<string, Mock>;
  controller: InternalController;
} {
  const inventario = { listar: vi.fn(), resumen: vi.fn() };
  const items = { encontrarPorId: vi.fn(), lotePorSkus: vi.fn() };
  const controller = new InternalController(
    inventario as unknown as InventarioService,
    items as unknown as ItemsService,
  );
  return { inventario, items, controller };
}

describe('InternalController (internal, R-U-10/11)', () => {
  it('debe dividir la query de codigos en lista limpia y consultar el lote', async () => {
    const { items, controller } = fabricar();
    items.lotePorSkus.mockResolvedValue([{ sku: 'SKU-01', stock: 10, estado: 'disponible' }]);
    await controller.lote('SKU-01, , SKU-02');
    expect(items.lotePorSkus).toHaveBeenCalledWith(['SKU-01', 'SKU-02']);
  });

  it('debe consultar el lote vacio cuando no llegan codigos', async () => {
    const { items, controller } = fabricar();
    items.lotePorSkus.mockResolvedValue([]);
    await controller.lote(undefined);
    expect(items.lotePorSkus).toHaveBeenCalledWith([]);
  });

  it('debe devolver el resumen de inventario y las lineas de la bodega', async () => {
    const { inventario, controller } = fabricar();
    const resumen = { productos: 1, agotados: 0, stock_total: 10, valor_inventario_cents: 1500000 };
    inventario.resumen.mockResolvedValue(resumen);
    inventario.listar.mockResolvedValue([]);
    await expect(controller.resumen()).resolves.toEqual(resumen);
    await expect(controller.lineas()).resolves.toEqual([]);
  });

  it('debe devolver un item unificado o lanzar NotFoundError', async () => {
    const { items, controller } = fabricar();
    items.encontrarPorId.mockResolvedValue(item);
    await expect(controller.itemPorId(item.item_id)).resolves.toEqual(item);
    items.encontrarPorId.mockResolvedValue(null);
    await expect(controller.itemPorId(item.item_id)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe proteger todas las rutas con la clave interna', () => {
    const guards = Reflect.getMetadata('__guards__', InternalController) as unknown[];
    expect(guards).toBeDefined();
    expect(guards.some((g) => g === ClaveInternaGuard)).toBe(true);
  });
});
