import { InventarioService } from './inventario.service';

/** H-06: niveles de stock de la bodega desde la proyeccion CQRS. */
describe('InventarioService (niveles de stock)', () => {
  it('debe exponer stock fisico, reservado y disponible con precio serializado cuando la vista devuelve filas', async () => {
    const pg = {
      query: jest.fn().mockResolvedValue([
        {
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
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('FROM catalog.stock_vista'));
  });

  it('debe devolver una lista vacia cuando no hay existencias', async () => {
    const pg = { query: jest.fn().mockResolvedValue([]) };
    const servicio = new InventarioService(pg as never);
    expect(await servicio.listar()).toEqual([]);
  });
});
