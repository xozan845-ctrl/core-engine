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
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('FROM catalog.items i'), []);
  });

  it('debe devolver el resumen de inventario por empresa', async () => {
    const pg = {
      queryOne: vi.fn().mockResolvedValue({ productos: 2, agotados: 1, stock_total: 7, valor_inventario_cents: 7000 }),
    };
    const servicio = new InventarioService(pg as never);
    await expect(servicio.resumen('e-1')).resolves.toEqual({
      productos: 2,
      agotados: 1,
      stock_total: 7,
      valor_inventario_cents: 7000,
    });
    expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('i.empresa_id = $1'), ['e-1']);
  });

  it('debe devolver ceros cuando el resumen no tiene filas', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue(null) };
    const servicio = new InventarioService(pg as never);
    await expect(servicio.resumen()).resolves.toEqual({
      productos: 0,
      agotados: 0,
      stock_total: 0,
      valor_inventario_cents: 0,
    });
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
    expect(sql).toContain('i.empresa_id = $1');
    expect(sql).toContain('parent_item_id');
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

describe('InventarioService · consumo FEFO', () => {
  it('consume primero el lote con vencimiento más próximo y divide la cantidad', async () => {
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [{ track_inventory: true }], rowCount: 1 })
        .mockResolvedValueOnce({
          rows: [
            { id: 'lote-1', codigo_lote: 'L-1', cantidad_disponible: 5 },
            { id: 'lote-2', codigo_lote: 'L-2', cantidad_disponible: 10 },
          ],
          rowCount: 2,
        })
        .mockResolvedValueOnce({ rows: [], rowCount: 1 })
        .mockResolvedValueOnce({ rows: [], rowCount: 1 })
        .mockResolvedValueOnce({ rows: [{ stock: 7 }], rowCount: 1 }),
    };
    const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
    const servicio = new InventarioService(pg as never);

    await expect(
      servicio.consumirLotes({ empresa_id: 'e-1', item_id: 'item-1', cantidad: 8 }),
    ).resolves.toEqual({
      consumidos: [
        { lote_id: 'lote-1', codigo_lote: 'L-1', cantidad: 5 },
        { lote_id: 'lote-2', codigo_lote: 'L-2', cantidad: 3 },
      ],
      total_consumido: 8,
    });
    expect(client.query).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('ORDER BY vence_en NULLS LAST'),
      ['e-1', 'item-1', null],
    );
  });

  it('rechaza cantidades inválidas, items sin inventario y stock insuficiente', async () => {
    const servicioInvalido = new InventarioService({ transaccion: vi.fn() } as never);
    await expect(
      servicioInvalido.consumirLotes({ empresa_id: 'e-1', item_id: 'item-1', cantidad: 0 }),
    ).rejects.toMatchObject({ codigo: 'CANTIDAD_INVALIDA' });

    const sinInventario = {
      query: vi.fn().mockResolvedValueOnce({ rows: [{ track_inventory: false }], rowCount: 1 }),
    };
    await expect(
      new InventarioService({ transaccion: vi.fn(async (fn: (c: typeof sinInventario) => unknown) => fn(sinInventario)) } as never)
        .consumirLotes({ empresa_id: 'e-1', item_id: 'item-1', cantidad: 1 }),
    ).rejects.toMatchObject({ codigo: 'INVENTARIO_NO_APLICA' });

    const insuficiente = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [{ track_inventory: true }], rowCount: 1 })
        .mockResolvedValueOnce({ rows: [{ id: 'lote-1', codigo_lote: 'L-1', cantidad_disponible: 2 }], rowCount: 1 })
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }),
    };
    await expect(
      new InventarioService({ transaccion: vi.fn(async (fn: (c: typeof insuficiente) => unknown) => fn(insuficiente)) } as never)
        .consumirLotes({ empresa_id: 'e-1', item_id: 'item-1', cantidad: 3 }),
    ).rejects.toMatchObject({ codigo: 'STOCK_LOTE_INSUFICIENTE' });
  });
});
