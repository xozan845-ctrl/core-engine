import { PedidosConsumer } from './pedidos.consumer';
import { EVENTOS, COLAS, EventoBus, OrdenData, DevolucionSolicitadaData } from '@core/shared';

/**
 * Consumidor de pedidos (Tabla 22): descuento atomico de stock al crear la
 * orden (RN-03), reversion en devoluciones (RN-06) e idempotencia por
 * event_id (doc 5.2). Consumer cableado con el handler real del bus.
 */
describe('PedidosConsumer de catalog (RN-03 / RN-06)', () => {
  const crear = async () => {
    const rabbit = {
      declararColas: jest.fn().mockResolvedValue(undefined),
      consumir: jest.fn().mockResolvedValue(undefined),
      activarReintento: jest.fn(),
    };
    const outbox = { insertar: jest.fn().mockResolvedValue(undefined) };
    const productos = {
      estaProcesado: jest.fn().mockResolvedValue(false),
      registrarProcesado: jest.fn().mockResolvedValue(undefined),
      reservarStock: jest.fn().mockResolvedValue({
        ok: true,
        fallidos: [],
        stock_restante: [{ sku: 'sku-01', stock_restante: 4 }],
      }),
      reintegrarStock: jest.fn().mockResolvedValue(undefined),
      stockActualDe: jest.fn().mockResolvedValue([{ sku: 'sku-01', stock_restante: 9 }]),
    };
    const consumer = new PedidosConsumer(rabbit as never, outbox as never, productos as never);
    await consumer.onModuleInit();
    return {
      rabbit,
      outbox,
      productos,
      dispatch: (e: EventoBus) => (rabbit.consumir as jest.Mock).mock.calls[0][1](e),
      cola: (rabbit.consumir as jest.Mock).mock.calls[0][0],
    };
  };

  const evento = <T>(tipo: string, data: T, event_id = 'evt-1'): EventoBus<T> =>
    ({ event_id, tipo: tipo as EventoBus['tipo'], ocurrido_en: '2026-09-01T00:00:00.000Z', data }) as EventoBus<T>;

  const ordenData = {
    order_id: 'o1',
    items: [{ oferta_id: 'of1', sku: 'SKU-01', producto_nombre: 'Teclado', cantidad: 2 }],
  } as unknown as OrdenData;

  it('debe suscribir la cola de pedidos del catalogo al iniciar', async () => {
    const { rabbit, cola } = await crear();
    expect(cola).toBe(COLAS.catalog_pedidos.cola);
    expect(rabbit.declararColas).toHaveBeenCalledTimes(1);
    expect(rabbit.activarReintento).toHaveBeenCalled();
  });

  it('debe reservar stock y emitir stock.reservado + stock.updated cuando la orden es nueva', async () => {
    const { dispatch, outbox, productos } = await crear();
    await dispatch(evento(EVENTOS.ORDER_CREATED, ordenData));

    expect(productos.reservarStock).toHaveBeenCalledWith('o1', [{ sku: 'SKU-01', cantidad: 2 }]);
    const emitidos = (outbox.insertar as jest.Mock).mock.calls.map((c) => c[0]);
    expect(emitidos).toContain(EVENTOS.STOCK_RESERVADO);
    expect(emitidos).toContain(EVENTOS.STOCK_UPDATED);
    expect(productos.registrarProcesado).toHaveBeenCalledWith('evt-1', EVENTOS.ORDER_CREATED);
  });

  it('debe emitir stock.fallido cuando la reserva es rechazada por stock insuficiente', async () => {
    const { dispatch, outbox, productos } = await crear();
    (productos.reservarStock as jest.Mock).mockResolvedValue({
      ok: false,
      fallidos: [{ sku: 'SKU-01', cantidad: 2, motivo: 'stock_insuficiente' }],
    });

    await dispatch(evento(EVENTOS.ORDER_CREATED, ordenData));

    const emitidos = (outbox.insertar as jest.Mock).mock.calls.map((c) => c[0]);
    expect(emitidos).toEqual([EVENTOS.STOCK_FALLIDO]);
    expect(emitidos).not.toContain(EVENTOS.STOCK_RESERVADO);
    expect(productos.registrarProcesado).toHaveBeenCalled();
  });

  it('debe ignorar el evento cuando el event_id ya fue procesado (idempotencia doc 5.2)', async () => {
    const { dispatch, outbox, productos } = await crear();
    (productos.estaProcesado as jest.Mock).mockResolvedValue(true);

    await dispatch(evento(EVENTOS.ORDER_CREATED, ordenData));

    expect(productos.reservarStock).not.toHaveBeenCalled();
    expect(outbox.insertar).not.toHaveBeenCalled();
  });

  it('debe reintegrar stock y emitir stock.reintegrado cuando se solicita una devolucion', async () => {
    const { dispatch, outbox, productos } = await crear();
    const data = {
      order_id: 'o1',
      motivo: 'tamano incorrecto',
      items: [{ sku: 'SKU-01', cantidad: 1 }],
    } as unknown as DevolucionSolicitadaData;

    await dispatch(evento(EVENTOS.DEVOLUCION_SOLICITADA, data));

    expect(productos.reintegrarStock).toHaveBeenCalledWith('o1', [{ sku: 'SKU-01', cantidad: 1 }]);
    const emitidos = (outbox.insertar as jest.Mock).mock.calls.map((c) => c[0]);
    expect(emitidos).toContain(EVENTOS.STOCK_REINTEGRADO);
    expect(emitidos).toContain(EVENTOS.STOCK_UPDATED);
    expect(productos.stockActualDe).toHaveBeenCalledWith(['SKU-01']);
    expect(productos.registrarProcesado).toHaveBeenCalled();
  });

  it('debe ignorar los eventos que no son de pedidos', async () => {
    const { dispatch, outbox } = await crear();
    await dispatch(evento(EVENTOS.PAYMENT_PROCESADO, { order_id: 'o1', monto_cents: 1, estado: 'procesado', metodo: 'x' }));
    expect(outbox.insertar).not.toHaveBeenCalled();
  });
});
