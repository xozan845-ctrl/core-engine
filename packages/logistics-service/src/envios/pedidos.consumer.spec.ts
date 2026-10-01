import { PedidosConsumer } from './pedidos.consumer';
import {
  EVENTOS,
  COLAS,
  EventoBus,
  PaymentProcesadoData,
  OrderStatusUpdatedData,
} from '@core/shared';

/**
 * Consumidor de pagos/estados (Tabla 22): payment.procesado genera la guia
 * (shipment.started), entregada emite order.completado (comisiones devengan),
 * devuelta emite devolucion.solicitada (stock vuelve). Idempotencia doc 5.2.
 */
describe('PedidosConsumer de logistics (Tabla 22)', () => {
  const crear = async () => {
    const rabbit = {
      declararColas: jest.fn().mockResolvedValue(undefined),
      consumir: jest.fn().mockResolvedValue(undefined),
      activarReintento: jest.fn(),
    };
    const outbox = { insertar: jest.fn().mockResolvedValue(undefined) };
    const pg = {
      query: jest.fn().mockResolvedValue(undefined),
      queryOne: jest.fn().mockResolvedValue(null),
    };
    const envios = {
      prepararEnvio: jest.fn().mockResolvedValue({
        id: 'e1',
        guia: 'BGH-ABC-123456',
        order_id: 'o1',
        estado: 'en_preparacion',
        creado_en: '2026-09-01T00:00:00.000Z',
      }),
      consultarOrden: jest.fn().mockResolvedValue({
        id: 'o1',
        items: [{ sku: 'SKU-01', cantidad: 2 }],
        estado: 'devuelta',
      }),
    };
    const consumer = new PedidosConsumer(
      rabbit as never,
      outbox as never,
      pg as never,
      envios as never,
    );
    await consumer.onModuleInit();
    return {
      rabbit,
      outbox,
      pg,
      envios,
      cola: (rabbit.consumir as jest.Mock).mock.calls[0][0],
      dispatch: (e: EventoBus) => (rabbit.consumir as jest.Mock).mock.calls[0][1](e),
    };
  };

  const evento = (tipo: string, data: unknown, event_id = 'evt-1'): EventoBus => ({
    event_id,
    tipo: tipo as EventoBus['tipo'],
    ocurrido_en: '2026-09-01T00:00:00.000Z',
    data,
  });

  it('debe suscribir la cola de pagos del logistics al iniciar', async () => {
    const { rabbit, cola } = await crear();
    expect(cola).toBe(COLAS.logistics_pagos.cola);
    expect(rabbit.declararColas).toHaveBeenCalledTimes(1);
    expect(rabbit.activarReintento).toHaveBeenCalled();
  });

  it('debe preparar el envio y emitir shipment.started cuando el pago se procesa', async () => {
    const { dispatch, envios, outbox, pg } = await crear();
    const data = {
      order_id: 'o1',
      monto_cents: 150000,
      estado: 'procesado',
      metodo: 'simulado',
    } as PaymentProcesadoData;

    await dispatch(evento(EVENTOS.PAYMENT_PROCESADO, data));

    expect(envios.prepararEnvio).toHaveBeenCalledWith('o1', 150000);
    expect(outbox.insertar).toHaveBeenCalledWith(EVENTOS.SHIPMENT_STARTED, {
      order_id: 'o1',
      guia: 'BGH-ABC-123456',
      estado: 'en_preparacion',
    });
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('ON CONFLICT (event_id) DO NOTHING'),
      ['evt-1', EVENTOS.PAYMENT_PROCESADO],
    );
  });

  it('debe ignorar el pago cuando el event_id ya fue procesado (doc 5.2)', async () => {
    const { dispatch, envios, outbox, pg } = await crear();
    (pg.queryOne as jest.Mock).mockResolvedValue({ '?column?': 1 });
    await dispatch(
      evento(EVENTOS.PAYMENT_PROCESADO, {
        order_id: 'o1',
        monto_cents: 1,
        estado: 'procesado',
        metodo: 'x',
      }),
    );
    expect(envios.prepararEnvio).not.toHaveBeenCalled();
    expect(outbox.insertar).not.toHaveBeenCalled();
  });

  it('debe emitir order.completado cuando la orden queda entregada (comisiones devengan)', async () => {
    const { dispatch, outbox, envios } = await crear();
    const data = {
      order_id: 'o1',
      estado: 'entregada',
      previo_estado: 'en_camino',
    } as OrderStatusUpdatedData;

    await dispatch(evento(EVENTOS.ORDER_STATUS_UPDATED, data));

    const emitidos = (outbox.insertar as jest.Mock).mock.calls.map((c) => c[0]);
    expect(emitidos).toEqual([EVENTOS.ORDER_COMPLETADO]);
    expect(outbox.insertar).toHaveBeenCalledWith(
      EVENTOS.ORDER_COMPLETADO,
      expect.objectContaining({ order_id: 'o1' }),
    );
    expect(envios.consultarOrden).not.toHaveBeenCalled();
  });

  it('debe emitir devolucion.solicitada con los items de la orden cuando se devuelve', async () => {
    const { dispatch, outbox, envios } = await crear();
    const data = {
      order_id: 'o1',
      estado: 'devuelta',
      previo_estado: 'entregada',
      motivo: 'inspeccion rechazada',
    } as OrderStatusUpdatedData;

    await dispatch(evento(EVENTOS.ORDER_STATUS_UPDATED, data));

    expect(envios.consultarOrden).toHaveBeenCalledWith('o1');
    expect(outbox.insertar).toHaveBeenCalledWith(EVENTOS.DEVOLUCION_SOLICITADA, {
      order_id: 'o1',
      motivo: 'inspeccion rechazada',
      items: [{ sku: 'SKU-01', cantidad: 2 }],
    });
  });

  it('debe emitir devolucion.solicitada sin items cuando la consulta a la orden falla', async () => {
    const { dispatch, outbox, envios } = await crear();
    (envios.consultarOrden as jest.Mock).mockResolvedValue(null);
    await dispatch(
      evento(EVENTOS.ORDER_STATUS_UPDATED, {
        order_id: 'o1',
        estado: 'devuelta',
      } as OrderStatusUpdatedData),
    );
    expect(outbox.insertar).toHaveBeenCalledWith(EVENTOS.DEVOLUCION_SOLICITADA, {
      order_id: 'o1',
      motivo: 'devolucion aprobada', // motivo por defecto
      items: [],
    });
  });

  it('debe notificar y marcar sin emitir eventos de negocio cuando el estado es intermedio', async () => {
    const { dispatch, outbox, pg } = await crear();
    await dispatch(
      evento(EVENTOS.ORDER_STATUS_UPDATED, {
        order_id: 'o1',
        estado: 'pagada',
        previo_estado: 'creada',
      } as OrderStatusUpdatedData),
    );
    expect(outbox.insertar).not.toHaveBeenCalled();
    expect(pg.query).toHaveBeenCalledTimes(1); // solo marcarProcesado
  });

  it('debe ignorar los eventos que no son de pagos ni de estados', async () => {
    const { dispatch, outbox, pg } = await crear();
    await dispatch(evento(EVENTOS.ORDER_CREATED, { order_id: 'o1' }));
    expect(outbox.insertar).not.toHaveBeenCalled();
    expect(pg.queryOne).not.toHaveBeenCalled();
  });
});
