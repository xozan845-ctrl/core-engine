import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ejecutarConContexto } from '../logging/logger';

const { Pool } = vi.hoisted(() => ({ Pool: vi.fn() }));
vi.mock('pg', () => ({ Pool }));

import { PgService } from './pg.service';

type ClienteFalso = {
  query: ReturnType<typeof vi.fn>;
  release: ReturnType<typeof vi.fn>;
};

type PoolFalso = {
  query: ReturnType<typeof vi.fn>;
  on: ReturnType<typeof vi.fn>;
  connect: ReturnType<typeof vi.fn>;
  end: ReturnType<typeof vi.fn>;
};

function crearPool(): { pool: PoolFalso; client: ClienteFalso } {
  const client: ClienteFalso = {
    query: vi.fn(async () => ({ rows: [] })),
    release: vi.fn(),
  };
  const pool: PoolFalso = {
    query: vi.fn(async () => ({ rows: [] })),
    on: vi.fn(),
    connect: vi.fn(async () => client),
    end: vi.fn(async () => undefined),
  };
  // Debe ser una función normal: `new Pool()` no invoca funciones flecha.
  Pool.mockImplementation(function PoolFalsoMock() {
    return pool;
  } as never);
  return { pool, client };
}

let envOriginal: string | undefined;

beforeEach(() => {
  envOriginal = process.env.DATABASE_URL;
  delete process.env.DATABASE_URL;
  Pool.mockReset();
});

afterEach(() => {
  if (envOriginal === undefined) delete process.env.DATABASE_URL;
  else process.env.DATABASE_URL = envOriginal;
  vi.restoreAllMocks();
});

describe('PgService · ciclo de vida', () => {
  it('falla ruidosamente si falta DATABASE_URL', async () => {
    const servicio = new PgService({ url: '' });

    await expect(servicio.onModuleInit()).rejects.toThrow('DATABASE_URL no esta definida');
    expect(Pool).not.toHaveBeenCalled();
  });

  it('crea el pool, registra el handler de error y verifica la conexión', async () => {
    const { pool } = crearPool();
    const logError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const servicio = new PgService({ url: 'postgres://test' });

    await servicio.onModuleInit();

    expect(Pool).toHaveBeenCalledWith({ connectionString: 'postgres://test', max: 10 });
    expect(pool.query).toHaveBeenCalledWith('SELECT 1');
    const handler = pool.on.mock.calls.find(([evento]) => evento === 'error')?.[1] as (
      err: Error,
    ) => void;
    handler(new Error('conexión perdida'));
    expect(logError).toHaveBeenCalledWith(expect.stringContaining('conexión perdida'));
  });

  it('cierra el pool una sola vez', async () => {
    const { pool } = crearPool();
    const servicio = new PgService({ url: 'postgres://test' });
    await servicio.onModuleInit();

    await servicio.onModuleDestroy();
    await servicio.onModuleDestroy();

    expect(pool.end).toHaveBeenCalledOnce();
  });
});

describe('PgService · consultas', () => {
  it('rechaza consultas sin pool inicializado', async () => {
    const servicio = new PgService({ url: 'postgres://test' });

    await expect(servicio.query('SELECT 1')).rejects.toThrow('Pool no inicializado');
    await expect(servicio.transaccion(async () => 1)).rejects.toThrow('Pool no inicializado');
  });

  it('usa el pool directo cuando no hay contexto de usuario', async () => {
    const { pool } = crearPool();
    pool.query.mockResolvedValue({ rows: [{ total: 2 }] });
    const servicio = new PgService({ url: 'postgres://test' });
    await servicio.onModuleInit();

    await expect(servicio.query('SELECT 1', [1])).resolves.toEqual([{ total: 2 }]);

    expect(pool.query).toHaveBeenCalledWith('SELECT 1', [1]);
    expect(pool.connect).not.toHaveBeenCalled();
  });

  it('queryOne devuelve la primera fila o null', async () => {
    const { pool } = crearPool();
    const servicio = new PgService({ url: 'postgres://test' });
    await servicio.onModuleInit();

    pool.query.mockResolvedValueOnce({ rows: [{ id: 'a' }] });
    await expect(servicio.queryOne('SELECT 1')).resolves.toEqual({ id: 'a' });

    pool.query.mockResolvedValueOnce({ rows: [] });
    await expect(servicio.queryOne('SELECT 1')).resolves.toBeNull();
  });

  it('fuerza transacción y propaga los claims cuando hay usuario', async () => {
    const { pool, client } = crearPool();
    client.query.mockImplementation(async (sql: string) => {
      if (sql === 'SELECT 1') return { rows: [{ id: 'a' }] };
      return { rows: [] };
    });
    const servicio = new PgService({ url: 'postgres://test' });
    await servicio.onModuleInit();

    const filas = await ejecutarConContexto(
      {
        request_id: 'req-1',
        servicio: 'test',
        usuario_id: 'u-1',
        rol: 'empresario',
        tenant_id: 't-1',
      },
      () => servicio.query('SELECT 1'),
    );

    expect(filas).toEqual([{ id: 'a' }]);
    expect(client.query).toHaveBeenCalledWith('BEGIN');
    expect(client.query).toHaveBeenCalledWith(
      `SELECT set_config('request.jwt.claims', $1, true)`,
      [JSON.stringify({ sub: 'u-1', rol: 'empresario', tenant_id: 't-1' })],
    );
    expect(client.query).toHaveBeenCalledWith('SELECT 1', []);
    expect(client.query).toHaveBeenCalledWith('COMMIT');
    expect(client.release).toHaveBeenCalledOnce();
    expect(pool.connect).toHaveBeenCalledOnce();
  });

  it('hace COMMIT y libera el cliente al terminar la transacción', async () => {
    const { client } = crearPool();
    const servicio = new PgService({ url: 'postgres://test' });
    await servicio.onModuleInit();

    await expect(
      servicio.transaccion(async (c) => {
        await c.query('UPDATE x');
        return 42;
      }),
    ).resolves.toBe(42);

    expect(client.query).toHaveBeenNthCalledWith(1, 'BEGIN');
    expect(client.query).toHaveBeenCalledWith('UPDATE x');
    expect(client.query).toHaveBeenCalledWith('COMMIT');
    expect(client.release).toHaveBeenCalledOnce();
  });

  it('hace ROLLBACK y libera el cliente si la transacción falla', async () => {
    const { client } = crearPool();
    const servicio = new PgService({ url: 'postgres://test' });
    await servicio.onModuleInit();

    await expect(
      servicio.transaccion(async () => {
        throw new Error('fallo de negocio');
      }),
    ).rejects.toThrow('fallo de negocio');

    expect(client.query).toHaveBeenCalledWith('ROLLBACK');
    expect(client.release).toHaveBeenCalledOnce();
  });

  it('no inyecta claims cuando no hay usuario en contexto', async () => {
    const { client } = crearPool();
    const servicio = new PgService({ url: 'postgres://test' });
    await servicio.onModuleInit();

    await servicio.transaccion(async (c) => c.query('SELECT 1'));

    expect(client.query).not.toHaveBeenCalledWith(
      `SELECT set_config('request.jwt.claims', $1, true)`,
      expect.anything(),
    );
  });
});
