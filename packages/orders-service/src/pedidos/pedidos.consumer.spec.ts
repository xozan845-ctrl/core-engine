import type { Mock } from 'vitest';
import { PedidosConsumer } from './pedidos.consumer';
import { COLAS, EVENTOS, EventoBus } from '@core/shared';

describe('PedidosConsumer de resultados de stock (Tabla 22)', () => {
  const crear = async () => {
    const rabbit = {
      declararColas: vi.fn().mockResolvedValue(undefined),
      consumir: vi.fn().mockResolvedValue(undefined),
      activarReintento: vi.fn(),
    };
    const pg = {
      query: vi.fn().mockResolvedValue(undefined),
      queryOne: vi.fn().mockResolvedValue(null),
    };
    const handler = {
      transicionar: vi.fn().mockResolvedValue({ total_cents: 2500 }),
    };
    const consumer = new PedidosConsumer(rabbit as never, pg as never, handler as never);
    await consumer.onModuleInit();
    return {
      rabbit,
      pg,
      handler,
      cola: (rabbit.consumir as Mock).mock.calls[0][0],
      dispatch: (e: EventoBus) => (rabbit.consumir as Mock).mock.calls[0][1](e),
    };
  };

  const evento = (tipo: string, data: unknown, event_id = 'evt-1'): EventoBus => ({
    event_id,
    tipo: tipo as EventoBus['tipo'],
    ocurrido_en: '2026-09-01T00:00:00.000Z',
    data,
  });

  it('debe declarar y consumir la cola de resultados al iniciar', async () => {
    const { rabbit, cola } = await crear();

    expect(cola).toBe(COLAS.orders_resultados.cola);
    expect(rabbit.declararColas).toHaveBeenCalledWith([
      expect.objectContaining({ nombre: COLAS.orders_resultados.cola }),
    ]);
    expect(rabbit.activarReintento).toHaveBeenCalledTimes(1);
  });

  it('debe transicionar a pagada y emitir payment.procesado dentro de la misma transición', async () => {
    const { dispatch, handler, pg } = await crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ estado: 'creada', total_cents: 2500 });

    await dispatch(evento(EVENTOS.STOCK_RESERVADO, {
      order_id: 'order-1',
      items: [{ sku: 'SKU-01', cantidad: 2 }],
    }));

    expect(handler.transicionar).toHaveBeenCalledWith(
      'order-1',
      'pagada',
      'pago_simulado_aprobado',
      'inventario',
      {
        tipo: EVENTOS.PAYMENT_PROCESADO,
        data: {
          order_id: 'order-1',
          monto_cents: 2500,
          estado: 'procesado',
          metodo: 'simulado',
        },
      },
    );
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('ON CONFLICT (event_id) DO NOTHING'),
      ['evt-1', EVENTOS.STOCK_RESERVADO],
    );
  });

  it('debe marcar stock reservado sin transicionar si la orden ya no esta creada', async () => {
    const { dispatch, handler, pg } = await crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ estado: 'pagada', total_cents: 2500 });

    await dispatch(evento(EVENTOS.STOCK_RESERVADO, { order_id: 'order-1', items: [] }));

    expect(handler.transicionar).not.toHaveBeenCalled();
    expect(pg.query).toHaveBeenCalledTimes(1);
  });

  it('debe marcar stock reservado aunque la orden no exista en la vista', async () => {
    const { dispatch, handler, pg } = await crear();

    await dispatch(evento(EVENTOS.STOCK_RESERVADO, { order_id: 'missing', items: [] }));

    expect(handler.transicionar).not.toHaveBeenCalled();
    expect(pg.query).toHaveBeenCalledTimes(1);
  });

  it('debe ignorar stock reservado ya procesado por event_id', async () => {
    const { dispatch, handler, pg } = await crear();
    (pg.queryOne as Mock).mockResolvedValue({ '?column?': 1 });

    await dispatch(evento(EVENTOS.STOCK_RESERVADO, { order_id: 'order-1', items: [] }));

    expect(handler.transicionar).not.toHaveBeenCalled();
    expect(pg.query).not.toHaveBeenCalled();
  });

  it('debe transicionar a cancelada con el motivo de stock y marcar stock fallido', async () => {
    const { dispatch, handler, pg } = await crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ estado: 'creada', total_cents: 0 });

    await dispatch(evento(EVENTOS.STOCK_FALLIDO, {
      order_id: 'order-2',
      items: [
        { sku: 'SKU-01', cantidad: 2, motivo: 'sin stock' },
        { sku: 'SKU-02', cantidad: 1, motivo: 'bloqueado' },
      ],
    }));

    expect(handler.transicionar).toHaveBeenCalledWith(
      'order-2',
      'cancelada',
      'stock_insuficiente: SKU-01, SKU-02',
      'inventario',
    );
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('ON CONFLICT (event_id) DO NOTHING'),
      ['evt-1', EVENTOS.STOCK_FALLIDO],
    );
  });

  it('debe marcar stock fallido sin transicionar si la orden no esta creada', async () => {
    const { dispatch, handler, pg } = await crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ estado: 'cancelada', total_cents: 0 });

    await dispatch(evento(EVENTOS.STOCK_FALLIDO, {
      order_id: 'order-2',
      items: [{ sku: 'SKU-01', cantidad: 1, motivo: 'sin stock' }],
    }));

    expect(handler.transicionar).not.toHaveBeenCalled();
    expect(pg.query).toHaveBeenCalledTimes(1);
  });

  it('debe ignorar stock fallido ya procesado por event_id', async () => {
    const { dispatch, handler, pg } = await crear();
    (pg.queryOne as Mock).mockResolvedValue({ '?column?': 1 });

    await dispatch(evento(EVENTOS.STOCK_FALLIDO, {
      order_id: 'order-2',
      items: [{ sku: 'SKU-01', cantidad: 1, motivo: 'sin stock' }],
    }));

    expect(handler.transicionar).not.toHaveBeenCalled();
    expect(pg.query).not.toHaveBeenCalled();
  });

  it('debe ignorar eventos distintos de stock reservado o fallido', async () => {
    const { dispatch, handler, pg } = await crear();

    await dispatch(evento(EVENTOS.ORDER_CREATED, { order_id: 'order-1' }));

    expect(handler.transicionar).not.toHaveBeenCalled();
    expect(pg.queryOne).not.toHaveBeenCalled();
  });
});
