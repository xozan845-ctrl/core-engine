import { EVENTOS } from '@core/shared';
import type { EventoBus, PgService, RabbitService } from '@core/shared';
import type { NotificacionesService } from './notificaciones.service';
import { PedidosConsumer } from './pedidos.consumer';

const EVENTO: EventoBus = {
  event_id: '33333333-3333-3333-3333-333333333333',
  tipo: EVENTOS.PAYMENT_PROCESADO,
  ocurrido_en: '2026-11-11T00:00:00.000Z',
  data: { order_id: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee' },
};

async function montar(yaProcesado: boolean) {
  let manejador: ((evento: EventoBus) => Promise<void>) | undefined;
  const rabbit = {
    declararColas: vi.fn().mockResolvedValue(undefined),
    consumir: vi.fn().mockImplementation((_cola: string, fn: typeof manejador) => {
      manejador = fn;
      return Promise.resolve();
    }),
    activarReintento: vi.fn(),
  };
  const pg = {
    queryOne: vi.fn().mockResolvedValue(yaProcesado ? { '?column?': 1 } : null),
    query: vi.fn().mockResolvedValue([]),
  };
  const notificaciones = { procesar: vi.fn().mockResolvedValue(undefined) };
  const consumer = new PedidosConsumer(
    rabbit as unknown as RabbitService,
    pg as unknown as PgService,
    notificaciones as unknown as NotificacionesService,
  );

  await consumer.onModuleInit();
  return { consumer, rabbit, pg, notificaciones, manejador: manejador! };
}

describe('PedidosConsumer', () => {
  it('declara la cola de notificaciones y activa el reintento', async () => {
    const { rabbit } = await montar(false);
    expect(rabbit.declararColas).toHaveBeenCalledWith([
      expect.objectContaining({ nombre: 'notifications.todos' }),
    ]);
    expect(rabbit.activarReintento).toHaveBeenCalled();
  });

  it('procesa y marca el evento cuando no se había visto', async () => {
    const { manejador, notificaciones, pg } = await montar(false);

    await manejador(EVENTO);

    expect(notificaciones.procesar).toHaveBeenCalledWith(EVENTO);
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO notifications.eventos_procesados'),
      [EVENTO.event_id, EVENTO.tipo],
    );
  });

  it('descarta un evento ya procesado (idempotencia)', async () => {
    const { manejador, notificaciones, pg } = await montar(true);

    await manejador(EVENTO);

    expect(notificaciones.procesar).not.toHaveBeenCalled();
    expect(pg.query).not.toHaveBeenCalled();
  });

  it('no marca el evento si el envío falla', async () => {
    const { manejador, notificaciones, pg } = await montar(false);
    notificaciones.procesar.mockRejectedValue(new Error('SMTP caído'));

    await expect(manejador(EVENTO)).rejects.toThrow('SMTP caído');
    expect(pg.query).not.toHaveBeenCalled();
  });
});
