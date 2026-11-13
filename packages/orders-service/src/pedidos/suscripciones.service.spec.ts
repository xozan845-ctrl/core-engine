import type { Mock } from 'vitest';
import { ConflictError, DomainError, NotFoundError } from '@core/shared';
import { SuscripcionesService } from './suscripciones.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const SUSCRIPCION = '22222222-2222-4222-8222-222222222222';
const OFERTA = '33333333-3333-4333-8333-333333333333';
const COMPRADOR = '44444444-4444-4444-8444-444444444444';
const ACTOR = '55555555-5555-4555-8555-555555555555';

const fila = {
  id: SUSCRIPCION,
  empresa_id: EMPRESA,
  oferta_id: OFERTA,
  item_id: null,
  cantidad: 1,
  comprador_id: COMPRADOR,
  cliente_parte_id: null,
  tipo_orden: 'compra' as const,
  metodo_pago: 'simulado' as const,
  fulfillment: 'none',
  intervalo: 'mensual' as const,
  estado: 'activa' as const,
  proxima_ejecucion: '2026-12-01',
  ultima_ejecucion: null,
  fin_en: null,
  metadata: {},
  creado_por: ACTOR,
  creado_en: '2026-11-13T00:00:00.000Z',
  actualizado_en: '2026-11-13T00:00:00.000Z',
};

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn() };
  const handler = { ejecutar: vi.fn() };
  return {
    servicio: new SuscripcionesService(pg as never, handler as never),
    pg,
    handler,
  };
}

describe('SuscripcionesService (Fase P2.3)', () => {
  it('lista y obtiene suscripciones', async () => {
    const { servicio, pg } = crear();
    (pg.query as Mock).mockResolvedValue([fila]);
    await expect(servicio.listar(EMPRESA, { estado: 'activa', limite: '10' })).resolves.toEqual([
      expect.objectContaining({ id: SUSCRIPCION }),
    ]);

    (pg.queryOne as Mock).mockResolvedValueOnce(fila);
    await expect(servicio.obtener(EMPRESA, SUSCRIPCION)).resolves.toMatchObject({
      intervalo: 'mensual',
    });
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.obtener(EMPRESA, SUSCRIPCION)).rejects.toBeInstanceOf(NotFoundError);
    await expect(servicio.obtener(EMPRESA, 'no-uuid')).rejects.toBeInstanceOf(DomainError);
  });

  it('crea una suscripcion valida', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(fila);
    await expect(
      servicio.crear(EMPRESA, ACTOR, {
        oferta_id: OFERTA,
        intervalo: 'mensual',
        proxima_ejecucion: '2026-12-01',
      }),
    ).resolves.toMatchObject({ comprador_id: COMPRADOR });
    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.suscripciones'),
      [
        EMPRESA, OFERTA, null, 1, ACTOR, null, 'simulado', 'none', 'mensual',
        '2026-12-01', null, '{}', ACTOR,
      ],
    );
  });

  it('valida cantidad, intervalo, fulfillment, fechas y credito', async () => {
    const { servicio } = crear();
    await expect(
      servicio.crear(EMPRESA, ACTOR, { oferta_id: OFERTA, intervalo: 'mensual', cantidad: 0 }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, ACTOR, { oferta_id: OFERTA, intervalo: 'diario' as never }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, ACTOR, {
        oferta_id: OFERTA, intervalo: 'mensual', fulfillment: 'dron',
      }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, ACTOR, {
        oferta_id: OFERTA, intervalo: 'mensual',
        proxima_ejecucion: '2026-12-10', fin_en: '2026-12-01',
      }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, ACTOR, {
        oferta_id: OFERTA, intervalo: 'mensual', metodo_pago: 'credito',
      }),
    ).rejects.toMatchObject({ codigo: 'CREDITO_REQUIERE_PARTE' });
    await expect(
      servicio.crear(EMPRESA, ACTOR, {
        oferta_id: OFERTA, intervalo: 'mensual', metadata: [] as never,
      }),
    ).rejects.toBeInstanceOf(DomainError);
  });

  it('pausa, reanuda y cancela con transiciones validas', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(fila)
      .mockResolvedValueOnce({ ...fila, estado: 'pausada' });
    await expect(servicio.pausar(EMPRESA, SUSCRIPCION)).resolves.toMatchObject({ estado: 'pausada' });

    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ ...fila, estado: 'pausada' })
      .mockResolvedValueOnce({ ...fila, estado: 'activa' });
    await expect(servicio.reanudar(EMPRESA, SUSCRIPCION)).resolves.toMatchObject({ estado: 'activa' });

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(fila)
      .mockResolvedValueOnce({ ...fila, estado: 'cancelada' });
    await expect(servicio.cancelar(EMPRESA, SUSCRIPCION)).resolves.toMatchObject({
      estado: 'cancelada',
    });

    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, estado: 'cancelada' });
    await expect(servicio.pausar(EMPRESA, SUSCRIPCION)).rejects.toBeInstanceOf(ConflictError);
  });

  it('renueva y crea la orden con la oferta y cantidad actuales', async () => {
    const { servicio, pg, handler } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, proxima_ejecucion: '2027-01-01' });
    handler.ejecutar.mockResolvedValue({ id: 'order-1' });

    await expect(servicio.renovar(EMPRESA, SUSCRIPCION, true)).resolves.toMatchObject({
      order_id: 'order-1',
    });
    expect(handler.ejecutar).toHaveBeenCalledWith(
      expect.objectContaining({
        tipo_orden: 'compra',
        items: [{ oferta_id: OFERTA, cantidad: 1, fulfillment: 'none' }],
        metodo_pago: 'simulado',
      }),
      COMPRADOR,
    );
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.suscripcion_ejecuciones'),
      [EMPRESA, SUSCRIPCION, 'order-1', 'creada', ''],
    );
  });

  it('rechaza renovar una suscripcion no reclamable', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.renovar(EMPRESA, SUSCRIPCION, false)).rejects.toMatchObject({
      codigo: 'SUSCRIPCION_NO_RENOVABLE',
    });
  });

  it('registra el error y reprograma al fallar la orden', async () => {
    const { servicio, pg, handler } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, proxima_ejecucion: '2027-01-01' });
    handler.ejecutar.mockRejectedValue(new Error('oferta inactiva'));

    await expect(servicio.renovar(EMPRESA, SUSCRIPCION, true)).rejects.toThrow('oferta inactiva');
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('SET proxima_ejecucion = CURRENT_DATE'),
      [SUSCRIPCION, EMPRESA],
    );
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.suscripcion_ejecuciones'),
      [EMPRESA, SUSCRIPCION, null, 'error', 'oferta inactiva'],
    );
  });

  it('renueva las vencidas y lista ejecuciones', async () => {
    const { servicio, pg, handler } = crear();
    (pg.query as Mock).mockResolvedValueOnce([{ id: SUSCRIPCION, empresa_id: EMPRESA }]);
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, proxima_ejecucion: '2027-01-01' });
    handler.ejecutar.mockResolvedValue({ id: 'order-1' });
    await expect(servicio.renovarVencidas()).resolves.toBe(1);

    (pg.queryOne as Mock).mockResolvedValueOnce(fila);
    (pg.query as Mock).mockResolvedValueOnce([{
      id: 'e-1',
      suscripcion_id: SUSCRIPCION,
      order_id: 'order-1',
      estado: 'creada',
      error: '',
      ejecutada_en: '2026-11-13T00:00:00.000Z',
    }]);
    await expect(servicio.listarEjecuciones(EMPRESA, SUSCRIPCION)).resolves.toHaveLength(1);
  });
});
