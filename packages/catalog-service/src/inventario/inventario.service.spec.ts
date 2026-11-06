import type { Mock } from 'vitest';
import { InventarioService } from './inventario.service';

/** H-06: niveles de stock de la bodega desde la proyeccion CQRS. */
describe('InventarioService (niveles de stock)', () => {
  it('debe exponer stock fisico, reservado y disponible con precio serializado cuando la vista devuelve filas', async () => {
    const pg = {
      query: vi.fn().mockResolvedValue([
        {
          empresa_id: 'e-1',
          sku: 'SKU-01',
          nombre: 'Teclado',
          stock_fisico: 10,
          reservado: 3,
          disponible: 7,
          estado: 'disponible',
          precio_base_cents: 150000,
          valor_disponible_cents: 1050000,
          valor_total_cents: 1500000,
        },
      ]),
    };
    const servicio = new InventarioService(pg as never);

    const [linea] = await servicio.listar();

    expect(linea.sku).toBe('SKU-01');
    expect(linea.stock).toBe(10); // stock_fisico
    expect(linea.disponible).toBe(7);
    expect(linea.reservado).toBe(3);
    expect(linea.precio_base).toBe('1500.00');
    expect(linea.empresa_id).toBe('e-1');
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('FROM catalog.stock_vista'), []);
  });

  it('debe devolver una lista vacia cuando no hay existencias', async () => {
    const pg = { query: vi.fn().mockResolvedValue([]) };
    const servicio = new InventarioService(pg as never);
    expect(await servicio.listar()).toEqual([]);
  });

  it('debe filtrar por empresa y excluir los productos contenedores de variantes', async () => {
    const pg = { query: vi.fn().mockResolvedValue([]) };
    const servicio = new InventarioService(pg as never);

    await servicio.listar('e-1');

    const [sql, params] = (pg.query as Mock).mock.calls[0];
    expect(sql).toContain('s.empresa_id = $1');
    expect(sql).toContain('producto_padre_id');
    expect(params).toEqual(['e-1']);
  });

  it('crea proveedores y lotes con inventario activo', async () => {
    const pg = {
      query: vi.fn().mockResolvedValue([]),
      queryOne: vi.fn()
        .mockResolvedValueOnce({ id: 'prov-1', empresa_id: 'e-1', codigo: 'PROV-1', nombre: 'Proveedor', correo: '', telefono: '', activo: true })
        .mockResolvedValueOnce({ track_inventory: true })
        .mockResolvedValueOnce({ id: 'lote-1', empresa_id: 'e-1', item_id: 'item-1', proveedor_id: 'prov-1', codigo_lote: 'L-1', vence_en: '2027-01-01', cantidad_inicial: 10, cantidad_disponible: 10 }),
    };
    const servicio = new InventarioService(pg as never);

    await expect(servicio.crearProveedor({ empresa_id: 'e-1', codigo: 'prov-1', nombre: 'Proveedor' }))
      .resolves.toMatchObject({ codigo: 'PROV-1' });
    await expect(servicio.crearLote({ empresa_id: 'e-1', item_id: 'item-1', proveedor_id: 'prov-1', codigo_lote: 'L-1', cantidad_inicial: 10 }))
      .resolves.toMatchObject({ cantidad_disponible: 10 });
  });

  it('rechaza lotes para items que no controlan inventario y lista por item', async () => {
    const pg = {
      query: vi.fn().mockResolvedValue([{ id: 'lote-1' }]),
      queryOne: vi.fn().mockResolvedValue({ track_inventory: false }),
    };
    const servicio = new InventarioService(pg as never);
    await expect(servicio.crearLote({ empresa_id: 'e-1', item_id: 'item-1', codigo_lote: 'L-1', cantidad_inicial: 1 }))
      .rejects.toMatchObject({ codigo: 'INVENTARIO_NO_APLICA' });
    await expect(servicio.listarLotes('e-1', 'item-1')).resolves.toEqual([{ id: 'lote-1' }]);
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('item_id = $2'), ['e-1', 'item-1']);
  });

  it('expone errores de proveedor, item, cantidad y persistencia de lote', async () => {
    const pg = {
      query: vi.fn().mockResolvedValue([]),
      queryOne: vi.fn()
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce({ track_inventory: true })
        .mockResolvedValueOnce({ track_inventory: true })
        .mockResolvedValueOnce(null),
    };
    const servicio = new InventarioService(pg as never);
    await expect(servicio.crearProveedor({ empresa_id: 'e-1', codigo: 'P-1', nombre: 'Proveedor' }))
      .rejects.toMatchObject({ codigo: 'PROVEEDOR_NO_CREADO' });
    await expect(servicio.crearLote({ empresa_id: 'e-1', item_id: 'item-1', codigo_lote: 'L-1', cantidad_inicial: 1 }))
      .rejects.toMatchObject({ codigo: 'ITEM_NO_ENCONTRADO' });
    await expect(servicio.crearLote({ empresa_id: 'e-1', item_id: 'item-1', codigo_lote: 'L-1', cantidad_inicial: -1 }))
      .rejects.toMatchObject({ codigo: 'CANTIDAD_INVALIDA' });
    await expect(servicio.crearLote({ empresa_id: 'e-1', item_id: 'item-1', codigo_lote: 'L-1', cantidad_inicial: 1 }))
      .rejects.toMatchObject({ codigo: 'LOTE_NO_CREADO' });
  });
});
