import type { Mock } from 'vitest';
import { OutboxService } from './outbox.service';

function crear() {
  const pg = { query: vi.fn(), transaccion: vi.fn() };
  const rabbit = { publicar: vi.fn() };
  const servicio = new OutboxService(pg as never, rabbit as never, 'orders.outbox');
  return { servicio, pg, rabbit };
}

function fila(eventId: string) {
  return {
    event_id: eventId,
    payload: JSON.stringify({
      event_id: eventId,
      tipo: 'order.created',
      ocurrido_en: '2026-11-08T00:00:00.000Z',
      data: { order_id: 'o-1' },
    }),
  };
}

describe('OutboxService (ADR-03: reclamo atómico y publicación)', () => {
  it('debe insertar el evento en la transacción de negocio', async () => {
    const { servicio } = crear();
    const cliente = { query: vi.fn().mockResolvedValue({ rows: [] }) };

    const id = await servicio.insertarEnTransaccion(cliente as never, 'order.created', {
      order_id: 'o-1',
    });

    expect(id).toMatch(/^[0-9a-f-]{36}$/);
    const [sql, params] = (cliente.query as Mock).mock.calls[0];
    expect(sql).toContain('INSERT INTO orders.outbox');
    expect(sql).toContain("'pendiente'");
    expect(JSON.parse(params[2] as string)).toMatchObject({
      event_id: id,
      tipo: 'order.created',
      data: { order_id: 'o-1' },
    });
  });

  it('debe reclamar con SKIP LOCKED y marcar publicado cuando el broker confirma', async () => {
    const { servicio, pg, rabbit } = crear();
    pg.query
      .mockResolvedValueOnce([fila('e-1')])
      .mockResolvedValueOnce([]);
    rabbit.publicar.mockResolvedValue(true);

    await (servicio as unknown as { publicarPendientes: () => Promise<void> }).publicarPendientes();

    const [reclamo, params] = (pg.query as Mock).mock.calls[0];
    expect(reclamo).toContain('UPDATE orders.outbox');
    expect(reclamo).toContain('FOR UPDATE SKIP LOCKED');
    expect(reclamo).toContain("bloqueado_en < NOW() - INTERVAL '5 minutes'");
    expect(params).toBeUndefined();

    const [marcado, paramsMarcado] = (pg.query as Mock).mock.calls[1];
    expect(marcado).toContain('estado = \'publicado\'');
    expect(marcado).toContain('bloqueado_en = NULL');
    expect(paramsMarcado).toEqual(['e-1']);
    expect(rabbit.publicar).toHaveBeenCalledWith(
      expect.objectContaining({ event_id: 'e-1', tipo: 'order.created' }),
    );
  });

  it('debe liberar el lease cuando el broker no confirma o lanza error', async () => {
    const { servicio, pg, rabbit } = crear();
    pg.query
      .mockResolvedValueOnce([fila('e-1'), fila('e-2')])
      .mockResolvedValue([]);
    rabbit.publicar.mockResolvedValueOnce(false).mockRejectedValueOnce(new Error('broker caido'));

    await (servicio as unknown as { publicarPendientes: () => Promise<void> }).publicarPendientes();

    const liberaciones = (pg.query as Mock).mock.calls.filter((c) =>
      String(c[0]).includes('bloqueado_en = NULL'),
    );
    expect(liberaciones).toHaveLength(2);
    expect(liberaciones.map((c) => c[1])).toEqual([['e-1'], ['e-2']]);
  });

  it('no debe reclamar si el servicio está detenido', async () => {
    const { servicio, pg } = crear();
    (servicio as unknown as { detenido: boolean }).detenido = true;

    await (servicio as unknown as { publicarPendientes: () => Promise<void> }).publicarPendientes();

    expect(pg.query).not.toHaveBeenCalled();
  });
});
