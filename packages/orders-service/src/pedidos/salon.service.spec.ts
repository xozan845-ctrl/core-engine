import type { Mock } from 'vitest';
import { ConflictError, DomainError, NotFoundError } from '@core/shared';
import { SalonService } from './salon.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const MESA = '22222222-2222-4222-8222-222222222222';
const COMANDA = '33333333-3333-4333-8333-333333333333';
const ORDEN = '44444444-4444-4444-8444-444444444444';

const mesa = {
  id: MESA,
  empresa_id: EMPRESA,
  codigo: 'M-01',
  nombre: 'Mesa 1',
  capacidad: 4,
  zona: 'salon',
  estado: 'libre' as const,
  creado_en: '2026-11-13T00:00:00.000Z',
  actualizado_en: '2026-11-13T00:00:00.000Z',
};
const comanda = {
  id: COMANDA,
  empresa_id: EMPRESA,
  order_id: ORDEN,
  mesa_id: MESA,
  estacion: 'cocina',
  estado: 'pendiente' as const,
  notas: '',
  items_json: [{ sku: 'PLATO-1', nombre: 'Plato', cantidad: 2 }],
  creado_en: '2026-11-13T00:00:00.000Z',
  actualizado_en: '2026-11-13T00:00:00.000Z',
  listo_en: null,
  entregado_en: null,
};

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn() };
  return { servicio: new SalonService(pg as never), pg };
}

describe('SalonService (Fase P2.5)', () => {
  it('lista y crea mesas validas', async () => {
    const { servicio, pg } = crear();
    (pg.query as Mock).mockResolvedValueOnce([mesa]);
    await expect(servicio.listarMesas(EMPRESA, { zona: 'salon', estado: 'libre' }))
      .resolves.toEqual([mesa]);

    (pg.queryOne as Mock).mockResolvedValueOnce(mesa);
    await expect(
      servicio.crearMesa(EMPRESA, { codigo: 'M-01', nombre: 'Mesa 1', capacidad: 4 }),
    ).resolves.toEqual(mesa);
    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.mesas'),
      [EMPRESA, 'M-01', 'Mesa 1', 4, '', 'libre'],
    );
  });

  it('valida mesas y traduce duplicados', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockRejectedValueOnce({ code: '23505' });
    await expect(
      servicio.crearMesa(EMPRESA, { codigo: 'M-01', nombre: 'Mesa 1' }),
    ).rejects.toBeInstanceOf(ConflictError);
    await expect(
      servicio.crearMesa(EMPRESA, { codigo: 'M 01', nombre: 'Mesa 1' }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crearMesa(EMPRESA, { codigo: 'M-01', nombre: '' }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crearMesa(EMPRESA, { codigo: 'M-01', nombre: 'Mesa', capacidad: 0 }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crearMesa(EMPRESA, { codigo: 'M-01', nombre: 'Mesa', estado: 'rota' as never }),
    ).rejects.toBeInstanceOf(DomainError);
  });

  it('actualiza una mesa y valida su existencia', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(mesa)
      .mockResolvedValueOnce({ ...mesa, estado: 'ocupada' });
    await expect(
      servicio.actualizarMesa(EMPRESA, MESA, { estado: 'ocupada' }),
    ).resolves.toMatchObject({ estado: 'ocupada' });

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(
      servicio.actualizarMesa(EMPRESA, MESA, { nombre: 'Otra' }),
    ).rejects.toBeInstanceOf(NotFoundError);
    await expect(servicio.actualizarMesa(EMPRESA, 'no-uuid', {})).rejects.toBeInstanceOf(DomainError);
  });

  it('crea una comanda desde los items de la orden y ocupa la mesa', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(mesa)
      .mockResolvedValueOnce({
        items_json: [
          { empresa_id: EMPRESA, sku: 'PLATO-1', producto_nombre: 'Plato', cantidad: 2 },
        ],
      })
      .mockResolvedValueOnce(comanda);
    (pg.query as Mock).mockResolvedValueOnce({ rows: [] });

    await expect(
      servicio.crearComanda(EMPRESA, { order_id: ORDEN, mesa_id: MESA, estacion: 'cocina' }),
    ).resolves.toMatchObject({ estacion: 'cocina' });
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining("SET estado = 'ocupada'"),
      [MESA, EMPRESA],
    );
  });

  it('crea una comanda con items libres y valida faltantes', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(comanda);
    await expect(
      servicio.crearComanda(EMPRESA, {
        estacion: 'bar',
        items: [{ sku: 'BEB-1', nombre: 'Cafe', cantidad: 1 }],
      }),
    ).resolves.toMatchObject({ id: COMANDA });

    await expect(
      servicio.crearComanda(EMPRESA, { estacion: 'bar' }),
    ).rejects.toMatchObject({ codigo: 'COMANDA_SIN_ITEMS' });
    await expect(
      servicio.crearComanda(EMPRESA, {
        estacion: 'bar',
        items: [{ sku: '', nombre: 'Cafe', cantidad: 1 }],
      }),
    ).rejects.toBeInstanceOf(DomainError);

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(
      servicio.crearComanda(EMPRESA, { order_id: ORDEN, estacion: 'cocina' }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it('lista comandas y avanza su estado', async () => {
    const { servicio, pg } = crear();
    (pg.query as Mock).mockResolvedValueOnce([comanda]);
    await expect(
      servicio.listarComandas(EMPRESA, { estado: 'pendiente', estacion: 'cocina', mesa_id: MESA }),
    ).resolves.toHaveLength(1);

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(comanda)
      .mockResolvedValueOnce({ ...comanda, estado: 'en_preparacion' });
    await expect(
      servicio.actualizarComanda(EMPRESA, COMANDA, { estado: 'en_preparacion' }),
    ).resolves.toMatchObject({ estado: 'en_preparacion' });

    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ ...comanda, estado: 'lista' })
      .mockResolvedValueOnce({ ...comanda, estado: 'entregada', entregado_en: 'x' });
    await expect(
      servicio.actualizarComanda(EMPRESA, COMANDA, { estado: 'entregada' }),
    ).resolves.toMatchObject({ estado: 'entregada' });
  });

  it('rechaza transiciones invalidas y comandas inexistentes', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...comanda, estado: 'entregada' });
    await expect(
      servicio.actualizarComanda(EMPRESA, COMANDA, { estado: 'pendiente' }),
    ).rejects.toBeInstanceOf(ConflictError);

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(
      servicio.actualizarComanda(EMPRESA, COMANDA, { estado: 'lista' }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });
});
