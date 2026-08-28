import { EventsConsumer } from '../eventos/events.consumer';
import { EVENTOS, EventoBus, OrdenData, PaymentProcesadoData, ComisionAcreditadaData } from '@core/shared';
import type { PoolClient } from 'pg';

/**
 * Consumer de eventos (Tabla 22): cada evento se procesa UNA sola vez con
 * deduplicacion atomica (doc 5.2): el marcado de `eventos_procesados` vive en
 * la misma transaccion que el asiento y las metricas (ADR-03).
 */
describe('EventsConsumer (idempotencia atomica y contabilidad)', () => {
  const crear = async () => {
    const client = { query: jest.fn().mockResolvedValue({ rows: [] }) } as unknown as PoolClient;
    const pg = {
      queryOne: jest.fn().mockResolvedValue(null),
      query: jest.fn().mockResolvedValue([]),
      transaccion: jest.fn(async (fn: (c: PoolClient) => Promise<unknown>) => fn(client)),
    };
    const contabilidad = { registrarEnTransaccion: jest.fn().mockResolvedValue({ id: 'a1' }) };
    const metricas = {
      sumarMetrica: jest.fn().mockResolvedValue(undefined),
      incrementarVendedor: jest.fn().mockResolvedValue(undefined),
    };
    const rabbit = {
      declararColas: jest.fn().mockResolvedValue(undefined),
      consumir: jest.fn().mockResolvedValue(undefined),
      activarReintento: jest.fn(),
    };
    const servicio = new EventsConsumer(rabbit as never, pg as never, contabilidad as never, metricas as never);
    await servicio.onModuleInit();
    return {
      servicio,
      // el handler que el consumer registro en el bus (cableado real)
      dispatch: (e: EventoBus) => (rabbit.consumir as jest.Mock).mock.calls[0][1](e),
      pg,
      client,
      contabilidad,
      metricas,
    };
  };

  const evento = <T>(tipo: string, data: T): EventoBus<T> =>
    ({ event_id: 'evt-1', tipo: tipo as EventoBus['tipo'], ocurrido_en: '2026-08-01T00:00:00.000Z', data }) as EventoBus<T>;

  it('rechaza eventos ya procesados (dedup por event_id, sin tocar la contabilidad)', async () => {
    const { servicio, dispatch, pg, contabilidad, metricas } = await crear();
    (pg.queryOne as jest.Mock).mockResolvedValue({ event_id: 'evt-1' }); // ya procesado
    await dispatch(evento(EVENTOS.PAYMENT_PROCESADO, { order_id: 'o1', monto_cents: 45000, estado: 'procesado', metodo: 'simulado' }));
    expect(pg.transaccion).not.toHaveBeenCalled();
    expect(contabilidad.registrarEnTransaccion).not.toHaveBeenCalled();
    expect(metricas.sumarMetrica).not.toHaveBeenCalled();
  });

  it('pago: asiento Caja/Fondos + metricas + marcado de idempotencia en la misma transaccion', async () => {
    const { servicio, dispatch, client, contabilidad, metricas } = await crear();
    await dispatch(evento<PaymentProcesadoData>(EVENTOS.PAYMENT_PROCESADO, { order_id: 'o1', monto_cents: 45000, estado: 'procesado', metodo: 'simulado' }));

    expect(contabilidad.registrarEnTransaccion).toHaveBeenCalledTimes(1);
    const { detalles } = (contabilidad.registrarEnTransaccion as jest.Mock).mock.calls[0][1];
    expect(detalles).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ cuenta_codigo: '1.1.1', debe_cents: 45000 }),
        expect.objectContaining({ cuenta_codigo: '2.1.2', haber_cents: 45000 }),
      ]),
    );
    expect(metricas.sumarMetrica).toHaveBeenCalledWith(client, 'pedidos', 0, 1);
    expect(metricas.sumarMetrica).toHaveBeenCalledWith(client, 'gmv', 45000, 0);
    // dedup atomico: mismos argumentos que el insert de eventos_procesados
    const marcar = (client.query as jest.Mock).mock.calls
      .map((c) => c[0] as string)
      .find((sql) => sql.includes('eventos_procesados'));
    expect(marcar).toBeDefined();
    expect((client.query as jest.Mock).mock.calls.find((c) => (c[0] as string).includes('eventos_procesados'))[1]).toEqual(['evt-1', EVENTOS.PAYMENT_PROCESADO]);
  });

  it('comision.acreditada: realiza el ingreso (RN-04), cuenta al vendedor y marca idempotencia', async () => {
    const { servicio, dispatch, client, contabilidad, metricas } = await crear();
    await dispatch(evento<ComisionAcreditadaData>(EVENTOS.COMISION_ACREDITADA, { order_id: 'o1', monto_cents: 5400, monto_vendedor_cents: 39600, vendedor_id: 'v1' }));

    const { detalles } = (contabilidad.registrarEnTransaccion as jest.Mock).mock.calls[0][1];
    expect(detalles).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ cuenta_codigo: '2.1.2', debe_cents: 45000 }),
        expect.objectContaining({ cuenta_codigo: '4.1', haber_cents: 5400 }),
        expect.objectContaining({ cuenta_codigo: '2.1.1', haber_cents: 39600 }),
      ]),
    );
    expect(metricas.incrementarVendedor).toHaveBeenCalledWith(client, 'v1', { ventas: 1, monto_cents: 45000, comision_cents: 5400 });
    const marcar = (client.query as jest.Mock).mock.calls.find((c) => (c[0] as string).includes('eventos_procesados'));
    expect(marcar[1]).toEqual(['evt-1', EVENTOS.COMISION_ACREDITADA]);
  });

  it('order.created: alimenta el embudo (checkouts completados) con dedup en la misma tx', async () => {
    const { servicio, dispatch, client, metricas } = await crear();
    await dispatch(evento<OrdenData>(EVENTOS.ORDER_CREATED, { order_id: 'o1', cliente_id: 'c1', items: [], total_cents: 45000, estado: 'creada' }));
    expect(metricas.sumarMetrica).toHaveBeenCalledWith(client, 'checkouts_completados', 0, 1);
    expect((client.query as jest.Mock).mock.calls.find((c) => (c[0] as string).includes('eventos_procesados'))).toBeDefined();
  });

  it('devolucion.solicitada (RN-06): revierte la comision contra el acreedor del vendedor', async () => {
    const { servicio, dispatch, client, contabilidad, metricas } = await crear();
    jest.spyOn(globalThis, 'fetch').mockResolvedValue({
      ok: true,
      json: async () => ({ total_cents: 45000 }),
    } as unknown as Response);

    await dispatch(evento(EVENTOS.DEVOLUCION_SOLICITADA, { order_id: 'o1', motivo: 'no le gusto', items: [] }));
    const { detalles } = (contabilidad.registrarEnTransaccion as jest.Mock).mock.calls[0][1];
    expect(detalles).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ cuenta_codigo: '4.1', debe_cents: 5400 }), // 12 % de 45 000
        expect.objectContaining({ cuenta_codigo: '2.1.1', haber_cents: 5400 }),
      ]),
    );
    expect(metricas.sumarMetrica).toHaveBeenCalledWith(client, 'devoluciones', 0, 1);
    expect((client.query as jest.Mock).mock.calls.find((c) => (c[0] as string).includes('eventos_procesados'))).toBeDefined();
  });
});
