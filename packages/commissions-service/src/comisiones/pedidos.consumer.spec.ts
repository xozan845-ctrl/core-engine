import type { Mock } from 'vitest';
import { PedidosConsumer } from './pedidos.consumer';
import { COLAS, EVENTOS, EventoBus } from '@core/shared';

describe('PedidosConsumer de commissions (Tabla 22)', () => {
  const crear = async () => {
    const rabbit = {
      declararColas: vi.fn().mockResolvedValue(undefined),
      consumir: vi.fn().mockResolvedValue(undefined),
      activarReintento: vi.fn(),
    };
    const client = {
      query: vi.fn().mockImplementation((sql: string) =>
        Promise.resolve({ rows: [], rowCount: String(sql).includes('INSERT') ? 1 : 0 }),
      ),
    };
    const outbox = {
      insertar: vi.fn().mockResolvedValue(undefined),
      insertarEnTransaccion: vi.fn().mockResolvedValue(undefined),
    };
    const pg = {
      query: vi.fn().mockResolvedValue(undefined),
      queryOne: vi.fn().mockResolvedValue(null),
      transaccion: vi.fn(async (fn: (c: unknown) => Promise<unknown>) => fn(client)),
    };
    const comisiones = {
      registrarPago: vi.fn().mockResolvedValue(undefined),
      acreditarLinea: vi.fn().mockResolvedValue({
        comision_cents: 100,
        monto_vendedor_cents: 70,
        monto_empresario_cents: 30,
      }),
      compensar: vi.fn().mockResolvedValue(undefined),
    };
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        total_cents: 1000,
        items: [{
          oferta_id: 'of-1',
          vendedor_id: 'ven-1',
          empresa_id: 'emp-1',
          tienda_id: 'tie-1',
          participacion_porcentaje: 70,
          precio_unitario_cents: 500,
          cantidad: 2,
        }],
      }),
    });
    vi.stubGlobal('fetch', fetchMock);

    const consumer = new PedidosConsumer(
      rabbit as never,
      outbox as never,
      pg as never,
      comisiones as never,
    );
    await consumer.onModuleInit();
    return {
      rabbit,
      outbox,
      pg,
      client,
      comisiones,
      fetchMock,
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

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('debe suscribir las colas de pedidos y liquidacion al iniciar', async () => {
    const { rabbit, cola } = await crear();

    expect(cola).toBe(COLAS.commissions_pedidos.cola);
    expect(rabbit.declararColas).toHaveBeenCalledWith([
      expect.objectContaining({ nombre: COLAS.commissions_pedidos.cola }),
      expect.objectContaining({ nombre: COLAS.commissions_liquidacion.cola }),
    ]);
    expect(rabbit.activarReintento).toHaveBeenCalledTimes(1);
  });

  it('debe registrar el pago y marcar el evento dentro de la misma transacción', async () => {
    const { dispatch, comisiones, pg, client } = await crear();

    await dispatch(evento(EVENTOS.PAYMENT_PROCESADO, {
      order_id: 'order-1',
      monto_cents: 2500,
    }));

    expect(comisiones.registrarPago).toHaveBeenCalledWith('order-1', 2500, client);
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('ON CONFLICT (event_id) DO NOTHING'),
      ['evt-1', EVENTOS.PAYMENT_PROCESADO],
    );
    expect(pg.transaccion).toHaveBeenCalled();
  });

  it('debe acreditar cada linea y emitir comision.acreditada en la misma transacción', async () => {
    const { dispatch, comisiones, outbox, fetchMock, client } = await crear();

    await dispatch(evento(EVENTOS.ORDER_COMPLETADO, { order_id: 'order-1' }));

    expect(fetchMock).toHaveBeenCalledWith(
      'http://orders-service:3004/internal/orders/order-1',
      expect.objectContaining({
        headers: { 'x-internal-key': '' },
        signal: expect.any(AbortSignal),
      }),
    );
    expect(comisiones.acreditarLinea).toHaveBeenCalledWith({
      orderId: 'order-1',
      lineaClave: 'of-1',
      empresaId: 'emp-1',
      tiendaId: 'tie-1',
      vendedorId: 'ven-1',
      ventaCents: 1000,
      participacion: 70,
    }, client);
    expect(outbox.insertarEnTransaccion).toHaveBeenCalledWith(
      client,
      EVENTOS.COMISION_ACREDITADA,
      expect.objectContaining({ order_id: 'order-1', monto_cents: 100 }),
    );
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('ON CONFLICT (event_id) DO NOTHING'),
      ['evt-1', EVENTOS.ORDER_COMPLETADO],
    );
  });

  it('debe fallar si la orden completada no esta disponible', async () => {
    const { dispatch, outbox, fetchMock } = await crear();
    fetchMock.mockResolvedValue({ ok: false });

    await expect(dispatch(evento(EVENTOS.ORDER_COMPLETADO, { order_id: 'order-1' })))
      .rejects.toThrow('Orden no disponible para acreditar comision.');
    expect(outbox.insertarEnTransaccion).not.toHaveBeenCalled();
  });

  it('debe compensar una devolucion y marcar el evento en la misma transacción', async () => {
    const { dispatch, comisiones, client } = await crear();

    await dispatch(evento(EVENTOS.DEVOLUCION_SOLICITADA, {
      order_id: 'order-1',
      motivo: 'devolucion aprobada',
    }));

    expect(comisiones.compensar).toHaveBeenCalledWith('order-1', 'devolucion aprobada', client);
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('ON CONFLICT (event_id) DO NOTHING'),
      ['evt-1', EVENTOS.DEVOLUCION_SOLICITADA],
    );
  });

  it('debe ignorar eventos ya procesados y tipos desconocidos', async () => {
    const primero = await crear();
    primero.pg.queryOne.mockResolvedValue({ '?column?': 1 });
    await primero.dispatch(evento(EVENTOS.PAYMENT_PROCESADO, { order_id: 'order-1', monto_cents: 1 }));
    expect(primero.comisiones.registrarPago).not.toHaveBeenCalled();
    expect(primero.client.query).not.toHaveBeenCalled();

    const segundo = await crear();
    await segundo.dispatch(evento(EVENTOS.ORDER_CREATED, { order_id: 'order-1' }));
    expect(segundo.comisiones.registrarPago).not.toHaveBeenCalled();
    expect(segundo.outbox.insertarEnTransaccion).not.toHaveBeenCalled();
  });
});
