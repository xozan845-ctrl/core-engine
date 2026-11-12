import type { Mock } from 'vitest';
import { ConflictError, DomainError, NotFoundError } from '@core/shared';
import { UbicacionesService } from './ubicaciones.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const ITEM = '22222222-2222-4222-8222-222222222222';
const ORIGEN = '33333333-3333-4333-8333-333333333333';
const DESTINO = '44444444-4444-4444-8444-444444444444';

const ubicacion = {
  id: ORIGEN,
  empresa_id: EMPRESA,
  codigo: 'bodega-central',
  nombre: 'Bodega central',
  tipo: 'bodega' as const,
  direccion: '',
  predeterminada: true,
  activo: true,
  creado_en: '2026-11-12T00:00:00.000Z',
  actualizado_en: '2026-11-12T00:00:00.000Z',
};
const item = {
  id: ITEM,
  codigo: 'SKU-01',
  nombre: 'Teclado',
  stock: 10,
  track_inventory: true,
};
const filaStock = {
  ubicacion_id: ORIGEN,
  ubicacion_codigo: 'bodega-central',
  ubicacion_nombre: 'Bodega central',
  item_id: ITEM,
  sku: 'SKU-01',
  nombre: 'Teclado',
  stock: 4,
  stock_total: 10,
};

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn(), transaccion: vi.fn() };
  pg.transaccion.mockImplementation(async (fn: (client: unknown) => unknown) =>
    fn({ query: vi.fn() }),
  );
  return { servicio: new UbicacionesService(pg as never), pg };
}

describe('UbicacionesService (Fase P1.2)', () => {
  it('lista las ubicaciones de la empresa', async () => {
    const { servicio, pg } = crear();
    (pg.query as Mock).mockResolvedValue([ubicacion]);
    await expect(servicio.listar(EMPRESA)).resolves.toEqual([ubicacion]);
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('FROM catalog.ubicaciones'),
      [EMPRESA],
    );
  });

  it('crea la primera ubicacion y le asigna el stock existente', async () => {
    const { servicio, pg } = crear();
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [ubicacion] })
        .mockResolvedValueOnce({ rows: [{ total: 1 }] })
        .mockResolvedValueOnce({ rows: [] }),
    };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(
      servicio.crear(EMPRESA, { codigo: 'Bodega-Central', nombre: 'Bodega central', predeterminada: true }),
    ).resolves.toEqual(ubicacion);

    expect(client.query).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('SET predeterminada = false'),
      [EMPRESA],
    );
    expect(client.query).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('INSERT INTO catalog.ubicaciones'),
      [EMPRESA, 'bodega-central', 'Bodega central', 'bodega', '', true],
    );
    expect(client.query).toHaveBeenNthCalledWith(
      3,
      expect.stringContaining('COUNT(*)::int AS total'),
      [EMPRESA],
    );
    expect(client.query).toHaveBeenNthCalledWith(
      4,
      expect.stringContaining('INSERT INTO catalog.item_stock_ubicacion'),
      [EMPRESA, ORIGEN],
    );
  });

  it('no reasigna stock cuando ya existen ubicaciones', async () => {
    const { servicio, pg } = crear();
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [{ ...ubicacion, predeterminada: false }] })
        .mockResolvedValueOnce({ rows: [{ total: 2 }] }),
    };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await servicio.crear(EMPRESA, { codigo: 'sucursal', nombre: 'Sucursal' });
    expect(client.query).toHaveBeenCalledTimes(2);
  });

  it('traduce duplicados y valida los datos de la ubicacion', async () => {
    const { servicio, pg } = crear();
    (pg.transaccion as Mock).mockRejectedValueOnce({ code: '23505' });
    await expect(
      servicio.crear(EMPRESA, { codigo: 'bodega', nombre: 'Bodega' }),
    ).rejects.toBeInstanceOf(ConflictError);

    await expect(
      servicio.crear(EMPRESA, { codigo: 'Bodega!', nombre: 'Bodega' }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, { codigo: 'bodega', nombre: 'x' }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, { codigo: 'bodega', nombre: 'Bodega', tipo: 'oficina' as never }),
    ).rejects.toBeInstanceOf(DomainError);
  });

  it('actualiza una ubicacion y cambia la predeterminada', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...ubicacion, predeterminada: false });
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [{ ...ubicacion, predeterminada: true }] }),
    };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(
      servicio.actualizar(EMPRESA, ORIGEN, { nombre: 'Central', predeterminada: true }),
    ).resolves.toMatchObject({ predeterminada: true });

    expect(client.query).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('AND id <> $2'),
      [EMPRESA, ORIGEN],
    );
  });

  it('no permite desactivar una ubicacion con stock', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(ubicacion)
      .mockResolvedValueOnce({ total: 5 });
    await expect(
      servicio.actualizar(EMPRESA, ORIGEN, { activo: false }),
    ).rejects.toBeInstanceOf(ConflictError);
  });

  it('actualiza una ubicacion inactiva sin volver a marcarla predeterminada', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(ubicacion);
    const client = {
      query: vi.fn().mockResolvedValue({
        rows: [{ ...ubicacion, predeterminada: false, activo: false }],
      }),
    };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(
      servicio.actualizar(EMPRESA, ORIGEN, { activo: false, predeterminada: true }),
    ).resolves.toMatchObject({ activo: false, predeterminada: false });
    expect(client.query).toHaveBeenCalledTimes(1);
  });

  it('traduce conflictos y ubicaciones inexistentes al actualizar', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(ubicacion);
    (pg.transaccion as Mock).mockRejectedValueOnce({ code: '23505' });
    await expect(
      servicio.actualizar(EMPRESA, ORIGEN, { nombre: 'Otra' }),
    ).rejects.toBeInstanceOf(ConflictError);

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.listarStock(EMPRESA, ORIGEN)).rejects.toBeInstanceOf(NotFoundError);
    await expect(servicio.actualizar(EMPRESA, 'no-uuid', {})).rejects.toBeInstanceOf(DomainError);
  });

  it('lista el stock de una ubicacion', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(ubicacion);
    (pg.query as Mock).mockResolvedValue([filaStock]);
    await expect(servicio.listarStock(EMPRESA, ORIGEN)).resolves.toEqual([filaStock]);
  });

  it('devuelve el desglose de stock de un item', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    (pg.query as Mock).mockResolvedValue([
      { ubicacion_id: ORIGEN, codigo: 'bodega-central', nombre: 'Central', stock: 4 },
      { ubicacion_id: DESTINO, codigo: 'sucursal', nombre: 'Sucursal', stock: 6 },
    ]);
    await expect(servicio.stockDeItem(EMPRESA, ITEM)).resolves.toMatchObject({
      item_id: ITEM,
      stock_total: 10,
      total_ubicaciones: 10,
    });
  });

  it('ajusta el stock y sincroniza el total del item', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(ubicacion).mockResolvedValueOnce(item);
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [filaStock] }),
    };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(servicio.ajustarStock(EMPRESA, ORIGEN, ITEM, 4)).resolves.toEqual(filaStock);
    expect(client.query).toHaveBeenNthCalledWith(
      2,
      `SELECT set_config('catalog.stock_sync', 'on', true)`,
    );
    expect(client.query).toHaveBeenNthCalledWith(
      3,
      expect.stringContaining('UPDATE catalog.items'),
      [EMPRESA, ITEM],
    );
    expect(client.query).toHaveBeenNthCalledWith(
      4,
      `SELECT set_config('catalog.stock_sync', 'off', true)`,
    );
  });

  it('rechaza ajustes invalidos o items sin inventario', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(ubicacion)
      .mockResolvedValueOnce({ ...item, track_inventory: false });
    await expect(
      servicio.ajustarStock(EMPRESA, ORIGEN, ITEM, 1),
    ).rejects.toBeInstanceOf(DomainError);

    (pg.queryOne as Mock).mockResolvedValueOnce({ ...ubicacion, activo: false });
    await expect(
      servicio.ajustarStock(EMPRESA, ORIGEN, ITEM, 1),
    ).rejects.toBeInstanceOf(ConflictError);

    (pg.queryOne as Mock).mockResolvedValueOnce(ubicacion).mockResolvedValueOnce(item);
    await expect(
      servicio.ajustarStock(EMPRESA, ORIGEN, ITEM, -1),
    ).rejects.toBeInstanceOf(DomainError);
  });

  it('traslada stock entre ubicaciones sin cambiar el total', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(item)
      .mockResolvedValueOnce(ubicacion)
      .mockResolvedValueOnce({ ...ubicacion, id: DESTINO, codigo: 'sucursal', nombre: 'Sucursal' });
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [{ stock: 10 }] })
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({
          rows: [
            { ...filaStock, stock: 7 },
            { ...filaStock, ubicacion_id: DESTINO, stock: 3 },
          ],
        }),
    };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(
      servicio.trasladar(EMPRESA, {
        item_id: ITEM,
        origen_id: ORIGEN,
        destino_id: DESTINO,
        cantidad: 3,
      }),
    ).resolves.toMatchObject({
      origen: { stock: 7 },
      destino: { ubicacion_id: DESTINO, stock: 3 },
    });
    expect(client.query).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('stock = stock - $1'),
      [3, EMPRESA, ITEM, ORIGEN],
    );
  });

  it('rechaza traslados sin stock, al mismo destino o con cantidad invalida', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(item)
      .mockResolvedValueOnce(ubicacion)
      .mockResolvedValueOnce({ ...ubicacion, id: DESTINO });
    const client = { query: vi.fn().mockResolvedValueOnce({ rows: [{ stock: 1 }] }) };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(
      servicio.trasladar(EMPRESA, {
        item_id: ITEM,
        origen_id: ORIGEN,
        destino_id: DESTINO,
        cantidad: 5,
      }),
    ).rejects.toBeInstanceOf(ConflictError);

    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    await expect(
      servicio.trasladar(EMPRESA, {
        item_id: ITEM,
        origen_id: ORIGEN,
        destino_id: ORIGEN,
        cantidad: 1,
      }),
    ).rejects.toBeInstanceOf(DomainError);
    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    await expect(
      servicio.trasladar(EMPRESA, {
        item_id: ITEM,
        origen_id: ORIGEN,
        destino_id: DESTINO,
        cantidad: 0,
      }),
    ).rejects.toBeInstanceOf(DomainError);
  });
});
