import type { Mock } from 'vitest';
import { OrderEventStore } from './order-event-store';

describe('OrderEventStore (concurrencia optimista)', () => {
  it('debe reintentar el append cuando la versión choca (23505)', async () => {
    const pg = {
      queryOne: vi
        .fn()
        .mockRejectedValueOnce({ code: '23505' })
        .mockResolvedValueOnce({ event_id: 'e-2', aggregate_id: 'o-1', version: 2 }),
    };
    const store = new OrderEventStore(pg as never);

    const fila = await store.append('o-1', 'OrderStatusUpdatedEvent', { estado: 'pagada' });

    expect(fila.version).toBe(2);
    expect(pg.queryOne).toHaveBeenCalledTimes(2);
  });

  it('debe propagar un error que no sea de unicidad', async () => {
    const pg = { queryOne: vi.fn().mockRejectedValue(new Error('db caida')) };
    const store = new OrderEventStore(pg as never);

    await expect(store.append('o-1', 'OrderStatusUpdatedEvent', {})).rejects.toThrow('db caida');
  });

  it('debe usar savepoint y reintentar dentro de la transacción', async () => {
    let inserts = 0;
    const client = {
      query: vi.fn().mockImplementation((sql: string) => {
        if (sql.startsWith('INSERT')) {
          inserts += 1;
          if (inserts === 1) return Promise.reject({ code: '23505' });
          return Promise.resolve({ rows: [{ event_id: 'e-2', version: 2 }] });
        }
        return Promise.resolve({ rows: [] });
      }),
    };
    const store = new OrderEventStore({} as never);

    const fila = await store.appendEnTransaccion(
      client as never,
      'o-1',
      'OrderStatusUpdatedEvent',
      { estado: 'pagada' },
    );

    expect(fila.version).toBe(2);
    expect(inserts).toBe(2);
    const sqls = (client.query as Mock).mock.calls.map((c: unknown[]) => String(c[0]));
    expect(sqls).toContain('SAVEPOINT eventos_append');
    expect(sqls).toContain('ROLLBACK TO SAVEPOINT eventos_append');
    expect(sqls).toContain('RELEASE SAVEPOINT eventos_append');
  });

  it('debe leer la historia ordenada por versión', async () => {
    const pg = { query: vi.fn().mockResolvedValue([{ version: 1 }, { version: 2 }]) };
    const store = new OrderEventStore(pg as never);

    const historia = await store.historiaDe('o-1');

    expect(historia).toHaveLength(2);
    expect((pg.query as Mock).mock.calls[0][0]).toContain('ORDER BY version ASC');
  });
});
