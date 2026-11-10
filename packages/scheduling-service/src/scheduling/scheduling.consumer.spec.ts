import { describe, expect, it, vi } from 'vitest';
import { EVENTOS } from '@core/shared';
import { SchedulingConsumer } from './scheduling.consumer';

describe('SchedulingConsumer', () => {
  it('declara la cola, crea citas solo para reservas y deduplica eventos', async () => {
    const rabbit = {
      declararColas: vi.fn(),
      consumir: vi.fn(),
      activarReintento: vi.fn(),
    };
    const pg = { queryOne: vi.fn().mockResolvedValue(null), query: vi.fn().mockResolvedValue([]) };
    const scheduling = { crearDesdeReserva: vi.fn().mockResolvedValue({ id: 'cita-1' }) };
    const consumer = new SchedulingConsumer(rabbit as never, pg as never, scheduling as never);
    await consumer.onModuleInit();
    expect(rabbit.declararColas).toHaveBeenCalledOnce();
    const manejar = rabbit.consumir.mock.calls[0][1] as (evento: unknown) => Promise<void>;

    await manejar({ event_id: 'e-1', tipo: EVENTOS.ORDER_CREATED, data: { tipo_orden: 'compra' } });
    expect(scheduling.crearDesdeReserva).not.toHaveBeenCalled();
    await manejar({ event_id: 'e-2', tipo: EVENTOS.ORDER_CREATED, data: { tipo_orden: 'reserva', order_id: 'o-1', cliente_parte_id: 'p-1' } });
    expect(scheduling.crearDesdeReserva).toHaveBeenCalledOnce();

    pg.queryOne.mockResolvedValueOnce({ ok: 1 });
    await manejar({ event_id: 'e-3', tipo: EVENTOS.ORDER_CREATED, data: { tipo_orden: 'reserva' } });
    expect(pg.query).toHaveBeenCalledTimes(2);
  });

  it('ignora eventos que no son order.created', async () => {
    const rabbit = { declararColas: vi.fn(), consumir: vi.fn(), activarReintento: vi.fn() };
    const pg = { queryOne: vi.fn(), query: vi.fn() };
    const scheduling = { crearDesdeReserva: vi.fn() };
    const consumer = new SchedulingConsumer(rabbit as never, pg as never, scheduling as never);
    await consumer.onModuleInit();
    const manejar = rabbit.consumir.mock.calls[0][1] as (evento: unknown) => Promise<void>;
    await manejar({ event_id: 'e-4', tipo: EVENTOS.APPOINTMENT_CREATED, data: {} });
    expect(pg.queryOne).not.toHaveBeenCalled();
  });
});
