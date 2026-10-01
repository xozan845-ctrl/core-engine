import { StockConsumer } from './stock.consumer';
import { EVENTOS, COLAS, EventoBus, StockUpdatedData } from '@core/shared';

/**
 * RN-02: el stock.visible del catalogo se sincroniza con las ofertas del
 * stores y las agotadas pasan a "agotada". Idempotencia por event_id (doc 5.2).
 */
describe('StockConsumer de stores (RN-02)', () => {
  const crear = async () => {
    const rabbit = {
      declararColas: jest.fn().mockResolvedValue(undefined),
      consumir: jest.fn().mockResolvedValue(undefined),
      activarReintento: jest.fn(),
    };
    const pg = {
      query: jest.fn().mockResolvedValue(undefined),
      queryOne: jest.fn().mockResolvedValue(null),
    };
    const ofertas = { sincronizarStockDeOferta: jest.fn().mockResolvedValue(undefined) };
    const consumer = new StockConsumer(rabbit as never, pg as never, ofertas as never);
    await consumer.onModuleInit();
    return {
      rabbit,
      pg,
      ofertas,
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

  it('debe suscribir la cola de stock del stores al iniciar', async () => {
    const { rabbit, cola } = await crear();
    expect(cola).toBe(COLAS.stores_stock.cola);
    expect(rabbit.declararColas).toHaveBeenCalledTimes(1);
    expect(rabbit.activarReintento).toHaveBeenCalled();
  });

  it('debe sincronizar cada item y registrar el evento cuando llega stock.updated', async () => {
    const { dispatch, ofertas, pg } = await crear();
    const data = {
      tipo: 'reservado',
      items: [
        { sku: 'SKU-01', stock_restante: 5 },
        { sku: 'SKU-02', stock_restante: 0 },
      ],
    } as unknown as StockUpdatedData;

    await dispatch(evento(EVENTOS.STOCK_UPDATED, data));

    expect(ofertas.sincronizarStockDeOferta).toHaveBeenCalledTimes(2);
    expect(ofertas.sincronizarStockDeOferta).toHaveBeenNthCalledWith(1, 'SKU-01', 5);
    expect(ofertas.sincronizarStockDeOferta).toHaveBeenNthCalledWith(2, 'SKU-02', 0);
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('ON CONFLICT (event_id) DO NOTHING'),
      ['evt-1', EVENTOS.STOCK_UPDATED],
    );
  });

  it('debe ignorar el evento cuando el event_id ya fue procesado (doc 5.2)', async () => {
    const { dispatch, ofertas, pg } = await crear();
    (pg.queryOne as jest.Mock).mockResolvedValue({ '?column?': 1 });
    await dispatch(evento(EVENTOS.STOCK_UPDATED, { tipo: 'reservado', items: [] }));
    expect(ofertas.sincronizarStockDeOferta).not.toHaveBeenCalled();
    expect(pg.query).not.toHaveBeenCalled();
  });

  it('debe ignorar los eventos que no son stock.updated', async () => {
    const { dispatch, ofertas, pg } = await crear();
    await dispatch(evento(EVENTOS.ORDER_CREATED, { order_id: 'o1' }));
    expect(pg.queryOne).not.toHaveBeenCalled(); // ni siquiera consulta idempotencia
    expect(ofertas.sincronizarStockDeOferta).not.toHaveBeenCalled();
  });

  it('debe registrar el evento aunque venga sin items (nada que sincronizar)', async () => {
    const { dispatch, ofertas, pg } = await crear();
    await dispatch(evento(EVENTOS.STOCK_UPDATED, { tipo: 'reservado' }));
    expect(ofertas.sincronizarStockDeOferta).not.toHaveBeenCalled();
    expect(pg.query).toHaveBeenCalledTimes(1); // solo marcarProcesado
  });
});
