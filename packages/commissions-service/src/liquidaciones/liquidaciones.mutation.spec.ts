import { LiquidacionesService } from './liquidaciones.service';

function crear() {
  const pg = {
    query: vi.fn(),
    queryOne: vi.fn(),
  };
  return { servicio: new LiquidacionesService(pg as never), pg };
}

describe('LiquidacionesService (mutantes dirigidos)', () => {
  it('devuelve vacío y no consolida cuando no hay comisiones', async () => {
    const { servicio, pg } = crear();
    pg.query.mockResolvedValueOnce([]);

    const resultado = await servicio.cerrarPeriodo({
      inicio: '2026-08-01',
      fin: '2026-08-15',
    });

    expect(resultado).toEqual([]);
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('UPDATE commissions.comisiones'),
      ['2026-08-01', '2026-08-15'],
    );
    expect(pg.query).toHaveBeenCalledTimes(1);
  });

  it('consolida por vendedor, aplica compensaciones y omite montos cero o sin fila', async () => {
    const { servicio, pg } = crear();
    pg.query
      .mockResolvedValueOnce([
        { vendedor_id: 'v1', empresa_id: 'e1', monto_vendedor_cents: 100, monto_empresario_cents: 20 },
        { vendedor_id: 'v1', empresa_id: 'e1', monto_vendedor_cents: 50, monto_empresario_cents: 10 },
        { vendedor_id: 'v2', empresa_id: 'e2', monto_vendedor_cents: 200, monto_empresario_cents: 40 },
        { vendedor_id: 'v3', empresa_id: 'e3', monto_vendedor_cents: 10, monto_empresario_cents: 2 },
      ])
      .mockResolvedValueOnce([
        { vendedor_id: 'v1', total: 20 },
        { vendedor_id: 'v2', total: -200 },
      ])
      .mockResolvedValueOnce([
        { empresa_id: 'e1', total: 5 },
        { empresa_id: 'e2', total: -40 },
      ]);
    pg.queryOne
      .mockResolvedValueOnce({
        id: 'l1',
        vendedor_id: 'v1',
        periodo_inicio: '2026-08-01',
        periodo_fin: '2026-08-15',
        monto_cents: 170,
        estado: 'aprobada',
        creado_en: '2026-08-15T00:00:00.000Z',
      })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({
        id: 'le1',
        empresa_id: 'e1',
        periodo_inicio: '2026-08-01',
        periodo_fin: '2026-08-15',
        monto_cents: 35,
        estado: 'aprobada',
        creado_en: '2026-08-15T00:00:00.000Z',
      })
      .mockResolvedValueOnce(null);

    const resultado = await servicio.cerrarPeriodo({
      inicio: '2026-08-01',
      fin: '2026-08-15',
    });

    expect(resultado).toHaveLength(1);
    expect(resultado[0].vendedor_id).toBe('v1');
    expect(pg.queryOne).toHaveBeenCalledTimes(4);
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('INSERT INTO commissions.liquidaciones'),
      ['v1', '2026-08-01', '2026-08-15', 170],
    );
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('INSERT INTO commissions.liquidaciones'),
      ['v3', '2026-08-01', '2026-08-15', 10],
    );
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      3,
      expect.stringContaining('INSERT INTO commissions.liquidaciones_empresas'),
      ['e1', '2026-08-01', '2026-08-15', 35],
    );
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      4,
      expect.stringContaining('INSERT INTO commissions.liquidaciones_empresas'),
      ['e3', '2026-08-01', '2026-08-15', 2],
    );
    expect(pg.query).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('FROM commissions.compensaciones_devoluciones'),
      ['2026-08-01', '2026-08-15'],
    );
    expect(pg.query).toHaveBeenNthCalledWith(
      3,
      expect.stringContaining('GROUP BY empresa_id'),
      ['2026-08-01', '2026-08-15'],
    );
  });

  it('cerrarPeriodoAnterior usa el periodo que corresponde a la fecha', async () => {
    const { servicio } = crear();
    const spy = vi.spyOn(servicio, 'cerrarPeriodo').mockResolvedValue([]);

    await servicio.cerrarPeriodoAnterior(new Date(2026, 7, 20));

    expect(spy).toHaveBeenCalledWith({
      inicio: '2026-08-16',
      fin: '2026-08-31',
    });
  });

  it('deVendedor filtra por vendedor y ordena por periodo', async () => {
    const { servicio, pg } = crear();
    pg.query.mockResolvedValue([{ id: 'l1' }]);

    const resultado = await servicio.deVendedor('v1');

    expect(resultado).toEqual([{ id: 'l1' }]);
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('WHERE vendedor_id = $1'),
      ['v1'],
    );
  });

  it('marcarPagada actualiza la liquidación y devuelve la fila', async () => {
    const { servicio, pg } = crear();
    const liquidacion = { id: 'l1', estado: 'pagada' };
    pg.queryOne.mockResolvedValue(liquidacion);

    await expect(servicio.marcarPagada('l1')).resolves.toBe(liquidacion);
    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining("SET estado = 'pagada'"),
      ['l1'],
    );
  });

  it('formatea meses y días de un dígito y el fin de año', () => {
    expect(servicioPeriodo(new Date(2026, 0, 5))).toEqual({
      inicio: '2026-01-01',
      fin: '2026-01-15',
    });
    expect(servicioPeriodo(new Date(2026, 8, 9))).toEqual({
      inicio: '2026-09-01',
      fin: '2026-09-15',
    });
    expect(servicioPeriodo(new Date(2026, 11, 20))).toEqual({
      inicio: '2026-12-16',
      fin: '2026-12-31',
    });
  });

  it('periodoACerrar cubre el día 16+ del mes vigente', () => {
    const { servicio } = crear();
    expect(servicio.periodoACerrar(new Date(2026, 7, 20))).toEqual({
      inicio: '2026-08-16',
      fin: '2026-08-31',
    });
  });
});

function servicioPeriodo(fecha: Date) {
  const { servicio } = crear();
  return servicio.periodoDe(fecha);
}
