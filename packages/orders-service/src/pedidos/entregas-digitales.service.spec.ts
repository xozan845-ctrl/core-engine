import type { Mock } from 'vitest';
import { ConflictError, DomainError, ForbiddenError, NotFoundError, ROLES } from '@core/shared';
import { EntregasDigitalesService } from './entregas-digitales.service';

const ORDER = '11111111-1111-4111-8111-111111111111';
const TOKEN = '22222222-2222-4222-8222-222222222222';
const ITEM = '33333333-3333-4333-8333-333333333333';

const entrega = {
  id: '44444444-4444-4444-8444-444444444444',
  order_id: ORDER,
  item_id: ITEM,
  sku: 'DIG-01',
  token: TOKEN,
  estado: 'activo' as const,
  descargas: 0,
  max_descargas: 3,
  expira_en: null,
  primera_descarga_en: null,
  ultima_descarga_en: null,
  creado_en: '2026-11-13T00:00:00.000Z',
  recurso_url: 'https://cdn.example.com/curso.pdf',
  comprador_id: 'u-1',
};
const comprador = { user_id: 'u-1', rol: ROLES.COMPRADOR };
const admin = { user_id: 'u-admin', rol: ROLES.ADMIN };

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn() };
  return { servicio: new EntregasDigitalesService(pg as never), pg };
}

describe('EntregasDigitalesService (Fase P2.1)', () => {
  it('lista los accesos de una orden para su comprador o un admin', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ comprador_id: 'u-1' });
    (pg.query as Mock).mockResolvedValueOnce([entrega]);
    await expect(servicio.listarPorOrden(ORDER, comprador)).resolves.toEqual([entrega]);

    (pg.queryOne as Mock).mockResolvedValueOnce({ comprador_id: 'u-1' });
    (pg.query as Mock).mockResolvedValueOnce([entrega]);
    await expect(servicio.listarPorOrden(ORDER, admin)).resolves.toEqual([entrega]);
  });

  it('rechaza a otro comprador y las ordenes inexistentes', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ comprador_id: 'u-2' });
    await expect(
      servicio.listarPorOrden(ORDER, comprador),
    ).rejects.toBeInstanceOf(ForbiddenError);

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.listarPorOrden(ORDER, comprador)).rejects.toBeInstanceOf(NotFoundError);

    await expect(servicio.listarPorOrden('no-uuid', comprador)).rejects.toBeInstanceOf(DomainError);
  });

  it('consume un acceso pagado e incrementa el contador', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ ...entrega, order_estado: 'pagada' })
      .mockResolvedValueOnce({ ...entrega, descargas: 1, primera_descarga_en: '2026-11-13T01:00:00.000Z' });

    await expect(servicio.descargar(TOKEN, comprador)).resolves.toMatchObject({
      descargas: 1,
      recurso_url: 'https://cdn.example.com/curso.pdf',
    });
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('descargas = descargas + 1'),
      [TOKEN],
    );
  });

  it('falla cerrado si la orden no esta pagada, el acceso expiro o no quedan descargas', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...entrega, order_estado: 'creada' });
    await expect(servicio.descargar(TOKEN, comprador)).rejects.toMatchObject({
      codigo: 'ORDEN_NO_PAGADA',
    });

    (pg.queryOne as Mock).mockResolvedValueOnce({
      ...entrega,
      order_estado: 'pagada',
      estado: 'revocado',
    });
    await expect(servicio.descargar(TOKEN, comprador)).rejects.toMatchObject({
      codigo: 'ENTREGA_REVOCADA',
    });

    (pg.queryOne as Mock).mockResolvedValueOnce({
      ...entrega,
      order_estado: 'pagada',
      expira_en: '2020-01-01T00:00:00.000Z',
    });
    await expect(servicio.descargar(TOKEN, comprador)).rejects.toMatchObject({
      codigo: 'ENTREGA_EXPIRADA',
    });

    (pg.queryOne as Mock).mockResolvedValueOnce({
      ...entrega,
      order_estado: 'pagada',
      max_descargas: 1,
      descargas: 1,
    });
    await expect(servicio.descargar(TOKEN, comprador)).rejects.toMatchObject({
      codigo: 'ENTREGA_SIN_DESCARGAS',
    });
  });

  it('rechaza accesos inexistentes o ya agotados en la actualizacion', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.descargar(TOKEN, comprador)).rejects.toBeInstanceOf(NotFoundError);

    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ ...entrega, order_estado: 'pagada' })
      .mockResolvedValueOnce(null);
    await expect(servicio.descargar(TOKEN, comprador)).rejects.toBeInstanceOf(ConflictError);

    await expect(servicio.descargar('no-uuid', comprador)).rejects.toBeInstanceOf(DomainError);
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...entrega, order_estado: 'pagada' });
    await expect(servicio.descargar(TOKEN, { user_id: 'u-2', rol: ROLES.COMPRADOR }))
      .rejects.toBeInstanceOf(ForbiddenError);
  });
});
