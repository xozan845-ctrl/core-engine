import type { Mock } from 'vitest';
import { ConflictError, NotFoundError } from '@core/shared';
import { TarifasService } from './tarifas.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const ITEM = '22222222-2222-4222-8222-222222222222';
const TARIFA = '33333333-3333-4333-8333-333333333333';

const item = { id: ITEM, tipo: 'alquiler', precio_cents: 10000 };
const tarifa = {
  id: TARIFA,
  empresa_id: EMPRESA,
  item_id: ITEM,
  desde: '2026-12-20',
  hasta: '2026-12-31',
  precio_cents: 20000,
  creado_en: '2026-11-11T00:00:00.000Z',
  actualizado_en: '2026-11-11T00:00:00.000Z',
};

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn() };
  return { servicio: new TarifasService(pg as never), pg };
}

describe('TarifasService (Fase C)', () => {
  it('lista las tarifas del item de la empresa', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    (pg.query as Mock).mockResolvedValue([tarifa]);
    await expect(servicio.listar(EMPRESA, ITEM)).resolves.toEqual([tarifa]);
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('catalog.item_tarifas'), [EMPRESA, ITEM]);
  });

  it('crea una tarifa válida y traduce el solape a 409', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item).mockResolvedValueOnce(tarifa);
    await expect(
      servicio.crear(EMPRESA, ITEM, { desde: '2026-12-20', hasta: '2026-12-31', precio_cents: 20000 }),
    ).resolves.toEqual(tarifa);

    (pg.queryOne as Mock).mockResolvedValueOnce(item).mockRejectedValueOnce({ code: '23P01' });
    await expect(
      servicio.crear(EMPRESA, ITEM, { desde: '2026-12-25', hasta: '2026-12-28', precio_cents: 15000 }),
    ).rejects.toBeInstanceOf(ConflictError);
  });

  it('actualiza y elimina tarifas existentes', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(tarifa)
      .mockResolvedValueOnce({ ...tarifa, precio_cents: 25000 });
    await expect(
      servicio.actualizar(EMPRESA, ITEM, TARIFA, { precio_cents: 25000 }),
    ).resolves.toMatchObject({ precio_cents: 25000 });

    (pg.queryOne as Mock).mockResolvedValueOnce(tarifa).mockResolvedValueOnce({ id: TARIFA });
    await expect(servicio.eliminar(EMPRESA, ITEM, TARIFA)).resolves.toEqual({ ok: true });

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.eliminar(EMPRESA, ITEM, TARIFA)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('cotiza el rango agrupando días con el mismo precio', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    (pg.query as Mock).mockResolvedValue([tarifa]);

    const cotizacion = await servicio.cotizar(EMPRESA, ITEM, '2026-12-18', '2026-12-22');

    expect(cotizacion).toMatchObject({
      item_id: ITEM,
      dias: 5,
      total_cents: 80000,
      desglose: [
        { desde: '2026-12-18', hasta: '2026-12-19', dias: 2, precio_cents: 10000, subtotal_cents: 20000 },
        { desde: '2026-12-20', hasta: '2026-12-22', dias: 3, precio_cents: 20000, subtotal_cents: 60000 },
      ],
    });
  });

  it('rechaza rangos inválidos o demasiado largos', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue(item);

    await expect(servicio.cotizar(EMPRESA, ITEM, '2026-12-22', '2026-12-18')).rejects.toMatchObject({
      codigo: 'RANGO_INVALIDO',
    });
    await expect(servicio.cotizar(EMPRESA, ITEM, '2026-01-01', '2027-12-31')).rejects.toMatchObject({
      codigo: 'RANGO_INVALIDO',
    });
    await expect(servicio.cotizar(EMPRESA, ITEM, '18/12/2026', '2026-12-22')).rejects.toMatchObject({
      codigo: 'RANGO_INVALIDO',
    });
    await expect(
      servicio.crear(EMPRESA, ITEM, { desde: '2026-12-20', hasta: '2026-12-19', precio_cents: 1 }),
    ).rejects.toMatchObject({ codigo: 'RANGO_INVALIDO' });
    await expect(
      servicio.crear(EMPRESA, ITEM, { desde: '2026-12-20', hasta: '2026-12-31', precio_cents: -1 }),
    ).rejects.toMatchObject({ codigo: 'PRECIO_INVALIDO' });
  });

  it('falla si el item no existe en la empresa', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.listar(EMPRESA, ITEM)).rejects.toBeInstanceOf(NotFoundError);
  });
});
