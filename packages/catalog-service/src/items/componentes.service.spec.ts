import type { Mock } from 'vitest';
import { ConflictError, NotFoundError } from '@core/shared';
import { ComponentesService } from './componentes.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const PADRE = '22222222-2222-4222-8222-222222222222';
const HIJO = '33333333-3333-4333-8333-333333333333';
const REL = '44444444-4444-4444-8444-444444444444';

const padre = { id: PADRE, tipo: 'producto', codigo: 'PLATO', nombre: 'Plato', stock: 0 };
const hijo = { id: HIJO, tipo: 'producto', codigo: 'HARINA', nombre: 'Harina', stock: 10 };
const componente = {
  id: REL,
  empresa_id: EMPRESA,
  item_id: PADRE,
  componente_item_id: HIJO,
  codigo: 'HARINA',
  nombre: 'Harina',
  stock: 10,
  cantidad: 2,
  creado_en: '2026-11-11T00:00:00.000Z',
  actualizado_en: '2026-11-11T00:00:00.000Z',
};

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn() };
  return { servicio: new ComponentesService(pg as never), pg };
}

describe('ComponentesService (Fase D)', () => {
  it('lista la composición del item', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(padre);
    (pg.query as Mock).mockResolvedValue([componente]);
    await expect(servicio.listar(EMPRESA, PADRE)).resolves.toEqual([componente]);
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('catalog.item_componentes'), [EMPRESA, PADRE]);
  });

  it('crea un componente válido y traduce duplicados a 409', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(padre)
      .mockResolvedValueOnce(hijo)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(componente);
    await expect(
      servicio.crear(EMPRESA, PADRE, { componente_item_id: HIJO, cantidad: 2 }),
    ).resolves.toEqual(componente);

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(padre)
      .mockResolvedValueOnce(hijo)
      .mockResolvedValueOnce(null)
      .mockRejectedValueOnce({ code: '23505' });
    await expect(
      servicio.crear(EMPRESA, PADRE, { componente_item_id: HIJO, cantidad: 2 }),
    ).rejects.toBeInstanceOf(ConflictError);
  });

  it('rechaza auto-composición, ciclos y componentes no producto', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(padre);
    await expect(
      servicio.crear(EMPRESA, PADRE, { componente_item_id: PADRE, cantidad: 1 }),
    ).rejects.toMatchObject({ codigo: 'COMPONENTE_INVALIDO' });

    (pg.queryOne as Mock).mockResolvedValueOnce(padre).mockResolvedValueOnce(hijo).mockResolvedValueOnce({ uno: 1 });
    await expect(
      servicio.crear(EMPRESA, PADRE, { componente_item_id: HIJO, cantidad: 1 }),
    ).rejects.toBeInstanceOf(ConflictError);

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(padre)
      .mockResolvedValueOnce({ ...hijo, tipo: 'servicio' });
    await expect(
      servicio.crear(EMPRESA, PADRE, { componente_item_id: HIJO, cantidad: 1 }),
    ).rejects.toMatchObject({ codigo: 'COMPONENTE_INVALIDO' });

    (pg.queryOne as Mock).mockResolvedValueOnce(padre);
    await expect(
      servicio.crear(EMPRESA, PADRE, { componente_item_id: HIJO, cantidad: 0 }),
    ).rejects.toMatchObject({ codigo: 'COMPONENTE_CANTIDAD_INVALIDA' });
  });

  it('permite que un servicio consuma repuestos (padre servicio)', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ ...padre, tipo: 'servicio', codigo: 'REPARACION' })
      .mockResolvedValueOnce(hijo)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ ...componente, item_id: PADRE });
    await expect(
      servicio.crear(EMPRESA, PADRE, { componente_item_id: HIJO, cantidad: 1 }),
    ).resolves.toMatchObject({ item_id: PADRE, componente_item_id: HIJO });
  });

  it('actualiza la cantidad y elimina el componente', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(padre)
      .mockResolvedValueOnce({ id: REL })
      .mockResolvedValueOnce({ ...componente, cantidad: 5 });
    await expect(
      servicio.actualizar(EMPRESA, PADRE, REL, { cantidad: 5 }),
    ).resolves.toMatchObject({ cantidad: 5 });

    (pg.queryOne as Mock).mockResolvedValueOnce(padre).mockResolvedValueOnce({ id: REL });
    await expect(servicio.eliminar(EMPRESA, PADRE, REL)).resolves.toEqual({ ok: true });

    (pg.queryOne as Mock).mockResolvedValueOnce(padre).mockResolvedValueOnce(null);
    await expect(servicio.eliminar(EMPRESA, PADRE, REL)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('falla si el item padre no existe', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.listar(EMPRESA, PADRE)).rejects.toBeInstanceOf(NotFoundError);
  });
});
