import { LiquidacionesService } from '../liquidaciones/liquidaciones.service';

describe('LiquidacionesService (RN-07: corte quincenal dias 1 y 15)', () => {
  const servicio = (() => {
    const fake = {
      pg: { query: vi.fn(), queryOne: vi.fn() },
      logger: { info: vi.fn(), error: vi.fn() },
    };
    return new LiquidacionesService(fake.pg as never);
  })();

  it('el dia 10 pertenece a la primera quincena (1-15)', () => {
    const periodo = servicio.periodoDe(new Date(2026, 7, 10)); // 10 agosto 2026
    expect(periodo.inicio).toBe('2026-08-01');
    expect(periodo.fin).toBe('2026-08-15');
  });

  it('el dia 15 sigue perteneciendo a la primera quincena', () => {
    const periodo = servicio.periodoDe(new Date(2026, 7, 15));
    expect(periodo.inicio).toBe('2026-08-01');
    expect(periodo.fin).toBe('2026-08-15');
  });

  it('el dia 16 pertenece a la segunda quincena (16-fin de mes)', () => {
    const periodo = servicio.periodoDe(new Date(2026, 7, 20)); // 20 agosto 2026
    expect(periodo.inicio).toBe('2026-08-16');
    expect(periodo.fin).toBe('2026-08-31');
  });

  it('febrero: la segunda quincena cierra el dia 28 (no bisiesto)', () => {
    const periodo = servicio.periodoDe(new Date(2026, 1, 25)); // 25 feb 2026
    expect(periodo.fin).toBe('2026-02-28');
  });

  it('el corte del dia 1 cierra el periodo previo del mes anterior', () => {
    const periodo = servicio.periodoACerrar(new Date(2026, 7, 1)); // 1 agosto 2026
    expect(periodo.inicio).toBe('2026-07-16');
    expect(periodo.fin).toBe('2026-07-31');
  });

  it('el corte del dia 15 cierra la primera quincena vigente', () => {
    const periodo = servicio.periodoACerrar(new Date(2026, 7, 15)); // 15 agosto 2026
    expect(periodo.inicio).toBe('2026-08-01');
    expect(periodo.fin).toBe('2026-08-15');
  });

  it('liquida por separado el neto del vendedor y del empresario', async () => {
    const pg = {
      query: vi.fn()
        .mockResolvedValueOnce([
          {
            id: 'comision-1', vendedor_id: 'vendedor-1', empresa_id: 'empresa-1',
            monto_vendedor_cents: 5_280, monto_empresario_cents: 3_520,
          },
        ])
        .mockResolvedValueOnce([{ vendedor_id: 'vendedor-1', total: -280 }])
        .mockResolvedValueOnce([{ empresa_id: 'empresa-1', total: -520 }]),
      queryOne: vi.fn()
        .mockResolvedValueOnce({ id: 'liq-vendedor', vendedor_id: 'vendedor-1', monto_cents: 5_000 })
        .mockResolvedValueOnce(null),
    };
    const servicioConDb = new LiquidacionesService(pg as never);

    await expect(servicioConDb.cerrarPeriodo({ inicio: '2026-08-01', fin: '2026-08-15' }))
      .resolves.toEqual([{ id: 'liq-vendedor', vendedor_id: 'vendedor-1', monto_cents: 5_000 }]);

    expect(pg.queryOne).toHaveBeenNthCalledWith(
      1, expect.stringContaining('commissions.liquidaciones'),
      ['vendedor-1', '2026-08-01', '2026-08-15', 5_000],
    );
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      2, expect.stringContaining('commissions.liquidaciones_empresas'),
      ['empresa-1', '2026-08-01', '2026-08-15', 3_000],
    );
  });
});
