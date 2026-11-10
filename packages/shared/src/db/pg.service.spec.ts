import type { Mock } from 'vitest';
import { PgService } from './pg.service';
import { ejecutarConContexto } from '../logging/logger';

function crearPool() {
  const client = {
    query: vi.fn().mockResolvedValue({ rows: [{ id: 'fila' }] }),
    release: vi.fn(),
  };
  const pool = {
    query: vi.fn().mockResolvedValue({ rows: [{ id: 'fila' }] }),
    connect: vi.fn().mockResolvedValue(client),
    end: vi.fn().mockResolvedValue(undefined),
    on: vi.fn(),
  };
  const servicio = new PgService();
  (servicio as unknown as { pool: unknown }).pool = pool;
  return { servicio, pool, client };
}

describe('PgService (pool, transacciones y claims RLS)', () => {
  it('debe fallar al inicializar sin DATABASE_URL', async () => {
    const anterior = process.env.DATABASE_URL;
    delete process.env.DATABASE_URL;
    try {
      await expect(new PgService().onModuleInit()).rejects.toThrow(/DATABASE_URL/);
    } finally {
      if (anterior !== undefined) process.env.DATABASE_URL = anterior;
    }
  });

  it('debe rechazar consultas y transacciones sin pool inicializado', async () => {
    const servicio = new PgService();
    await expect(servicio.query('SELECT 1')).rejects.toThrow(/Pool no inicializado/);
    await expect(servicio.transaccion(async () => 1)).rejects.toThrow(/Pool no inicializado/);
  });

  it('debe consultar con el pool cuando no hay contexto de usuario', async () => {
    const { servicio, pool } = crearPool();

    await expect(servicio.query('SELECT 1')).resolves.toEqual([{ id: 'fila' }]);
    expect(pool.query).toHaveBeenCalledWith('SELECT 1', []);
    expect(pool.connect).not.toHaveBeenCalled();
  });

  it('debe abrir transacción e inyectar claims cuando hay usuario en contexto', async () => {
    const { servicio, pool, client } = crearPool();

    await ejecutarConContexto(
      { request_id: 'r-1', servicio: 'test', usuario_id: 'u-1', rol: 'vendedor', tenant_id: 't-1' },
      () => servicio.query('SELECT 2'),
    );

    expect(pool.connect).toHaveBeenCalled();
    const sqls = (client.query as Mock).mock.calls.map((c: unknown[]) => String(c[0]));
    expect(sqls[0]).toBe('BEGIN');
    expect(sqls[1]).toContain("set_config('request.jwt.claims'");
    expect(JSON.parse((client.query as Mock).mock.calls[1][1][0])).toMatchObject({
      sub: 'u-1',
      rol: 'vendedor',
      tenant_id: 't-1',
    });
    expect(sqls[sqls.length - 1]).toBe('COMMIT');
    expect(client.release).toHaveBeenCalled();
  });

  it('debe hacer ROLLBACK y propagar el error de la transacción', async () => {
    const { servicio, client } = crearPool();
    client.query.mockImplementation((sql: string) => {
      if (sql === 'SELECT falla') return Promise.reject(new Error('boom'));
      return Promise.resolve({ rows: [] });
    });

    await expect(servicio.transaccion((c) => c.query('SELECT falla'))).rejects.toThrow('boom');
    const sqls = (client.query as Mock).mock.calls.map((c: unknown[]) => String(c[0]));
    expect(sqls).toContain('ROLLBACK');
    expect(client.release).toHaveBeenCalled();
  });

  it('debe devolver la primera fila o null en queryOne', async () => {
    const { servicio, pool } = crearPool();
    await expect(servicio.queryOne('SELECT 1')).resolves.toEqual({ id: 'fila' });
    pool.query.mockResolvedValueOnce({ rows: [] });
    await expect(servicio.queryOne('SELECT 1')).resolves.toBeNull();
  });

  it('debe cerrar el pool al destruir el módulo', async () => {
    const { servicio, pool } = crearPool();
    await servicio.onModuleDestroy();
    expect(pool.end).toHaveBeenCalled();
    await expect(servicio.query('SELECT 1')).rejects.toThrow(/Pool no inicializado/);
  });
});
