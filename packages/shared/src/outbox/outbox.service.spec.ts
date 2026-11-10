import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PoolClient } from 'pg';
import type { PgService } from '../db/pg.service';
import type { RabbitService } from '../rabbitmq/rabbit.service';
import { EVENTOS, type EventoBus } from '../events/contracts';
import { ejecutarConContexto } from '../logging/logger';
import type { MetricsService } from '../metrics/metrics.service';
import { METRICAS_OUTBOX, OutboxService } from './outbox.service';

type FilaOutbox = { event_id: string; payload: string | EventoBus };

interface ClienteFalso {
  query: ReturnType<typeof vi.fn>;
}

interface MetricsFalso {
  registrarGauge: ReturnType<typeof vi.fn>;
  incrementarContador: ReturnType<typeof vi.fn>;
}

function crearServicio(
  filas: FilaOutbox[] = [],
  publicar = vi.fn().mockResolvedValue(true),
  metrics?: MetricsFalso,
) {
  const consultas: Array<{ sql: string; params?: unknown[] }> = [];
  const client: ClienteFalso = {
    query: vi.fn(async (sql: string, params?: unknown[]) => {
      consultas.push({ sql, params });
      if (sql.includes('COUNT(*)')) return { rows: [{ pendientes: filas.length }] };
      if (sql.includes('SELECT event_id')) return { rows: filas };
      return { rows: [] };
    }),
  };
  const pg = {
    query: vi.fn(),
    transaccion: vi.fn(async (fn: (c: PoolClient) => Promise<unknown>) =>
      fn(client as unknown as PoolClient),
    ),
  };
  const rabbit = { publicar };
  const servicio = new OutboxService(
    pg as unknown as PgService,
    rabbit as unknown as RabbitService,
    'orders.outbox',
    metrics as unknown as MetricsService | undefined,
  );
  return { servicio, pg, rabbit, client, consultas, metrics };
}

function publicarPendientes(servicio: OutboxService): Promise<void> {
  return (
    servicio as unknown as { publicarPendientes: () => Promise<void> }
  ).publicarPendientes();
}

function evento(event_id = 'event-1'): EventoBus {
  return {
    event_id,
    tipo: EVENTOS.ORDER_CREATED,
    ocurrido_en: '2026-11-10T12:00:00.000Z',
    data: { order_id: 'order-1' },
  };
}

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('OutboxService · publicación (R-AR-3, R-RB-3)', () => {
  it('publica y marca el evento dentro de la misma transacción', async () => {
    const mensaje = evento();
    const { servicio, rabbit, client, consultas } = crearServicio([
      { event_id: mensaje.event_id, payload: JSON.stringify(mensaje) },
    ]);

    await publicarPendientes(servicio);

    expect(rabbit.publicar).toHaveBeenCalledWith(mensaje);
    const update = consultas.find((c) => c.sql.includes('UPDATE orders.outbox'));
    expect(update).toEqual({
      sql: expect.stringContaining("SET estado = 'publicado'"),
      params: [mensaje.event_id],
    });
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('FOR UPDATE SKIP LOCKED'),
    );
  });

  it('reclama con lock y no descarta pendientes antiguos', async () => {
    const { servicio, consultas } = crearServicio();

    await publicarPendientes(servicio);

    const select = consultas.find((c) => c.sql.includes('SELECT event_id'));
    expect(select?.sql).toContain('FOR UPDATE SKIP LOCKED');
    expect(select?.sql).toContain('LIMIT 50');
    expect(select?.sql).not.toContain('INTERVAL');
  });

  it('mantiene pendiente el evento si el broker no confirma', async () => {
    const publicar = vi.fn().mockResolvedValue(false);
    const { servicio, consultas } = crearServicio(
      [{ event_id: 'event-1', payload: JSON.stringify(evento()) }],
      publicar,
    );

    await publicarPendientes(servicio);

    expect(publicar).toHaveBeenCalledOnce();
    expect(consultas.some((c) => c.sql.includes('UPDATE orders.outbox'))).toBe(false);
  });

  it('aísla el fallo de un evento y continúa con el siguiente', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const publicar = vi
      .fn()
      .mockRejectedValueOnce(new Error('broker caído'))
      .mockResolvedValueOnce(true);
    const primero = evento('event-1');
    const segundo = evento('event-2');
    const { servicio, consultas } = crearServicio(
      [
        { event_id: 'event-1', payload: JSON.stringify(primero) },
        { event_id: 'event-2', payload: JSON.stringify(segundo) },
      ],
      publicar,
    );

    await publicarPendientes(servicio);

    expect(publicar).toHaveBeenCalledTimes(2);
    expect(publicar).toHaveBeenNthCalledWith(1, primero);
    expect(publicar).toHaveBeenNthCalledWith(2, segundo);
    expect(consultas.some((c) => c.params?.[0] === 'event-2')).toBe(true);
    expect(error).toHaveBeenCalledWith(expect.stringContaining('event-1'));
  });

  it('acepta payload jsonb ya deserializado por pg', async () => {
    const mensaje = evento();
    const { servicio, rabbit } = crearServicio([{ event_id: 'event-1', payload: mensaje }]);

    await publicarPendientes(servicio);

    expect(rabbit.publicar).toHaveBeenCalledWith(mensaje);
  });

  it('no publica nada después de onModuleDestroy', async () => {
    const { servicio, pg } = crearServicio();
    servicio.onModuleDestroy();

    await publicarPendientes(servicio);

    expect(pg.transaccion).not.toHaveBeenCalled();
  });

  it('el poller publica al cumplirse el intervalo y para al destruirse', async () => {
    vi.useFakeTimers();
    const { servicio, pg } = crearServicio();
    servicio.onModuleInit();

    await vi.advanceTimersByTimeAsync(1000);
    expect(pg.transaccion).toHaveBeenCalledOnce();

    servicio.onModuleDestroy();
    await vi.advanceTimersByTimeAsync(1000);
    expect(pg.transaccion).toHaveBeenCalledOnce();
  });
});

describe('OutboxService · métricas (R-OB-3, R-TM-5)', () => {
  it('expone el backlog, los publicados y los fallos del relay', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const metrics: MetricsFalso = {
      registrarGauge: vi.fn(),
      incrementarContador: vi.fn(),
    };
    const publicar = vi
      .fn()
      .mockResolvedValueOnce(true)
      .mockResolvedValueOnce(false)
      .mockRejectedValueOnce(new Error('broker caído'));
    const filas = ['event-1', 'event-2', 'event-3'].map((event_id) => ({
      event_id,
      payload: JSON.stringify(evento(event_id)),
    }));
    const { servicio } = crearServicio(filas, publicar, metrics);

    await publicarPendientes(servicio);

    expect(metrics.registrarGauge).toHaveBeenCalledWith(METRICAS_OUTBOX.pendientes, 3);
    expect(metrics.incrementarContador).toHaveBeenCalledWith(METRICAS_OUTBOX.publicados);
    const fallos = metrics.incrementarContador.mock.calls.filter(
      ([nombre]) => nombre === METRICAS_OUTBOX.fallos,
    );
    expect(fallos).toHaveLength(2);
    expect(error).toHaveBeenCalled();
  });

  it('no falla si el servicio se construye sin MetricsService', async () => {
    const { servicio } = crearServicio([
      { event_id: 'event-1', payload: JSON.stringify(evento()) },
    ]);

    await expect(publicarPendientes(servicio)).resolves.toBeUndefined();
  });
});

describe('OutboxService · inserción y replay', () => {
  it('inserta el evento en la tabla configurada con correlación', async () => {
    const { servicio, client } = crearServicio();
    let eventId = '';

    await ejecutarConContexto(
      { request_id: 'req-1', servicio: 'test' },
      async () => {
        eventId = await servicio.insertarEnTransaccion(
          client as unknown as PoolClient,
          EVENTOS.ORDER_CREATED,
          { order_id: 'order-1' },
        );
      },
    );

    const insert = client.query.mock.calls.find(([sql]) =>
      String(sql).includes('INSERT INTO orders.outbox'),
    );
    expect(insert).toBeDefined();
    const payload = JSON.parse(String(insert?.[1]?.[2])) as EventoBus;
    expect(payload).toMatchObject({
      event_id: eventId,
      tipo: EVENTOS.ORDER_CREATED,
      request_id: 'req-1',
      data: { order_id: 'order-1' },
    });
    expect(payload.ocurrido_en).toEqual(expect.any(String));
  });

  it('insertar abre su propia transacción', async () => {
    const { servicio, pg, client } = crearServicio();

    await servicio.insertar(EVENTOS.ORDER_CREATED, { order_id: 'order-1' });

    expect(pg.transaccion).toHaveBeenCalledOnce();
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.outbox'),
      expect.any(Array),
    );
  });

  it('reenviar publica todos los eventos publicados', async () => {
    const { servicio, pg, rabbit } = crearServicio();
    const mensaje = evento();
    pg.query.mockResolvedValue([{ event_id: 'event-1', payload: JSON.stringify(mensaje) }]);

    await expect(servicio.reenviar()).resolves.toBe(1);

    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining("WHERE estado = 'publicado'"),
      [],
    );
    expect(rabbit.publicar).toHaveBeenCalledWith(mensaje);
  });

  it('reenviar por event_id acota la consulta y devuelve el conteo', async () => {
    const { servicio, pg } = crearServicio();
    pg.query.mockResolvedValue([]);

    await expect(servicio.reenviar('event-9')).resolves.toBe(0);

    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('WHERE event_id = $1'),
      ['event-9'],
    );
  });
});
