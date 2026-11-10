import type { Mock } from 'vitest';
import { FacturacionService } from './facturacion.service';
import { DomainError, NotFoundError } from '@core/shared';

describe('FacturacionService (Ley 822 / comprobantes DGI)', () => {
  const comprobante = {
    id: 'c-1',
    serie: 'F001',
    numero: 'F00000001',
    tipo: 'FACTURA',
    orden_id: 'o-1',
    base_gravada_cents: 45000,
    iva_cents: 6750,
    exento_cents: 0,
    total_cents: 51750,
    moneda: 'C$',
    estado: 'BORRADOR',
    datos_cliente: null,
    emitido_en: null,
    creado_en: '2026-09-01T00:00:00Z',
  };

  const crear = (emitir = false) => {
    const pg = {
      query: vi.fn().mockResolvedValue([]),
      queryOne: vi.fn(),
      transaccion: vi.fn(),
    };
    const outbox = { insertarEnTransaccion: vi.fn().mockResolvedValue(undefined) };
    const contabilidad = { registrarEnTransaccion: vi.fn().mockResolvedValue(undefined) };
    const prev = process.env.EMITIR_COMPROBANTES_FISCALES;
    process.env.EMITIR_COMPROBANTES_FISCALES = String(emitir);
    const servicio = new FacturacionService(pg as never, outbox as never, contabilidad as never);
    if (prev === undefined) delete process.env.EMITIR_COMPROBANTES_FISCALES;
    else process.env.EMITIR_COMPROBANTES_FISCALES = prev;
    return { servicio, pg, outbox, contabilidad };
  };

  const mockOrden = (total_cents = 45000) => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ total_cents }),
    }));
  };

  const mockTransaccion = (pg: { transaccion: Mock }, fila = comprobante) => {
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [fila] })
        .mockResolvedValueOnce({ rows: [] }),
    };
    pg.transaccion.mockImplementation(async (fn: (c: unknown) => Promise<unknown>) => fn(client));
    return client;
  };

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('debe listar comprobantes con filtros opcionales y orden descendente', async () => {
    const { servicio, pg } = crear();
    (pg.query as Mock).mockResolvedValue([comprobante]);

    await expect(servicio.listar('2026-09-01', undefined, 'BORRADOR')).resolves.toEqual([comprobante]);
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('ORDER BY creado_en DESC'),
      ['2026-09-01', null, 'BORRADOR'],
    );
  });

  it('debe devolver las series ordenadas', async () => {
    const { servicio, pg } = crear();
    const series = [{ serie: 'F001', tipo: 'FACTURA', secuencial_actual: 1 }];
    (pg.query as Mock).mockResolvedValue(series);

    await expect(servicio.series()).resolves.toEqual(series);
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('FROM finance.series_comprobantes ORDER BY serie'));
  });

  it('debe anular solo comprobantes todavía vigentes', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue({ ...comprobante, estado: 'ANULADO' });

    await expect(servicio.anular('c-1')).resolves.toEqual({ ...comprobante, estado: 'ANULADO' });
    expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining("estado IN ('BORRADOR', 'EMITIDO')"), ['c-1']);
  });

  it('debe rechazar la emision cuando la orden no existe', async () => {
    const { servicio, pg } = crear();
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false }));

    await expect(servicio.emitir('FACTURA', 'o-inexistente')).rejects.toBeInstanceOf(NotFoundError);
    expect(pg.queryOne).not.toHaveBeenCalled();
  });

  it('debe rechazar la emision cuando no hay serie activa', async () => {
    const { servicio, pg } = crear();
    mockOrden();
    (pg.queryOne as Mock).mockResolvedValue(null);

    await expect(servicio.emitir('FACTURA', 'o-1')).rejects.toBeInstanceOf(DomainError);
    await expect(servicio.emitir('FACTURA', 'o-1')).rejects.toThrow('No hay serie activa para FACTURA.');
  });

  it('debe crear un BORRADOR, consumir el secuencial y omitir asiento fiscal', async () => {
    const { servicio, pg, outbox, contabilidad } = crear(false);
    mockOrden();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ serie: 'F001', tipo: 'FACTURA', prefijo: 'F', secuencial_actual: 0, jurisdiccion: 'NI', activa: true })
      .mockResolvedValueOnce({ tasa_iva: 0.15 });
    const client = mockTransaccion(pg, comprobante);

    await expect(servicio.emitir('FACTURA', 'o-1', { parte_id: 'p-1' })).resolves.toEqual(comprobante);
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('UPDATE finance.series_comprobantes'),
      ['F001'],
    );
    expect(contabilidad.registrarEnTransaccion).not.toHaveBeenCalled();
    expect(outbox.insertarEnTransaccion).not.toHaveBeenCalled();
  });

  it('debe emitir el comprobante, registrar IVA y publicar el evento', async () => {
    const { servicio, pg, outbox, contabilidad } = crear(true);
    mockOrden();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ serie: 'F001', tipo: 'FACTURA', prefijo: 'F', secuencial_actual: 0, jurisdiccion: 'NI', activa: true })
      .mockResolvedValueOnce({ tasa_iva: 0.15 });
    const emitido = { ...comprobante, estado: 'EMITIDO', emitido_en: '2026-09-01T00:00:00Z' };
    mockTransaccion(pg, emitido);

    await expect(servicio.emitir('FACTURA', 'o-1')).resolves.toEqual(emitido);
    expect(contabilidad.registrarEnTransaccion).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({ concepto: 'IVA trasladado FACTURA F001 F00000001' }),
    );
    expect(outbox.insertarEnTransaccion).toHaveBeenCalledWith(
      expect.anything(),
      'comprobante.emitido',
      expect.objectContaining({
        comprobante_id: 'c-1',
        iva_cents: 6750,
        total_cents: 51750,
      }),
    );
  });
});
