import { KpisService } from './kpis.service';

type FilaMetrica = { periodo: string; tipo: string; monto_cents: number; cantidad: number };

describe('KpisService (tabla 8.8 y resumen fiscal)', () => {
  const periodo = '2026-09';

  const crear = () => {
    const pg = {
      query: vi.fn(),
      queryOne: vi.fn(),
    };
    const servicio = new KpisService(pg as never);
    return { pg, servicio };
  };

  const metricasBase = (sobre: Partial<Record<string, number>> = {}): FilaMetrica[] => [
    { periodo: `${periodo}-01`, tipo: 'gmv', monto_cents: 123456, cantidad: 0 },
    { periodo: `${periodo}-01`, tipo: 'pedidos', monto_cents: 0, cantidad: 17 },
    { periodo: `${periodo}-01`, tipo: 'entregados', monto_cents: 0, cantidad: 15 },
    { periodo: `${periodo}-01`, tipo: 'ingresos_comisiones', monto_cents: 12345, cantidad: 0 },
    { periodo: `${periodo}-01`, tipo: 'devoluciones', monto_cents: 0, cantidad: 2 },
    { periodo: `${periodo}-01`, tipo: 'carritos_iniciados', monto_cents: 0, cantidad: sobre.carritos ?? 8 },
    {
      periodo: `${periodo}-01`,
      tipo: 'checkouts_completados',
      monto_cents: 0,
      cantidad: sobre.checkouts ?? 3,
    },
    {
      periodo: `${periodo}-01`,
      tipo: 'liquidaciones_a_tiempo',
      monto_cents: 0,
      cantidad: sobre.aTiempo ?? 1,
    },
    { periodo: `${periodo}-01`, tipo: 'cortes_esperados', monto_cents: 0, cantidad: sobre.esperados ?? 2 },
  ];

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('calcula y formatea todos los KPIs del periodo solicitado', async () => {
    const { pg, servicio } = crear();
    pg.query
      .mockResolvedValueOnce(metricasBase())
      .mockResolvedValueOnce([{ vendedor_id: 'v-1' }, { vendedor_id: 'v-3' }])
      .mockResolvedValueOnce([{ vendedor_id: 'v-1' }])
      .mockResolvedValueOnce([
        { vendedor_id: 'v-1', ventas: 2, primera_venta_en: '2026-09-05T00:00:00' },
        { vendedor_id: 'v-2', ventas: 1, primera_venta_en: '2026-10-01T00:00:00' },
      ]);
    pg.queryOne.mockResolvedValueOnce({ n: 4 });
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true,
      json: vi.fn().mockResolvedValue([
        { id: 'v-1', creado_en: '2026-09-01T00:00:00' },
        { id: 'v-2', creado_en: '2026-09-01T00:00:00' },
      ]),
    }));

    await expect(servicio.kpis(periodo)).resolves.toEqual({
      periodo: `${periodo}-01`,
      gmv_cents: 123456,
      gmv: '1234.56',
      pedidos_pagados: 17,
      pedidos_entregados: 15,
      ingresos_comisiones_cents: 12345,
      ingresos_comisiones: '123.45',
      devoluciones: 2,
      vendedores_activos: 4,
      pedidos_por_vendedor: 4.25,
      activacion_primeros_15_dias: 0.5,
      tasa_devoluciones: 0.1333,
      churn_mensual: 0.5,
      conversion_carrito_checkout: 0.375,
      liquidacion_a_tiempo: 0.5,
    });
    expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('COUNT(DISTINCT vendedor_id)'));
  });

  it('usa ceros y null cuando no hay métricas o no existen vendedores elegibles', async () => {
    const { pg, servicio } = crear();
    pg.query
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([]);
    pg.queryOne.mockResolvedValueOnce(null);
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: vi.fn().mockResolvedValue([]) }));

    await expect(servicio.kpis(periodo)).resolves.toMatchObject({
      periodo: `${periodo}-01`,
      gmv_cents: 0,
      pedidos_pagados: 0,
      pedidos_entregados: 0,
      ingresos_comisiones_cents: 0,
      devoluciones: 0,
      vendedores_activos: 0,
      pedidos_por_vendedor: 0,
      activacion_primeros_15_dias: null,
      tasa_devoluciones: 0,
      churn_mensual: 0,
      conversion_carrito_checkout: null,
      liquidacion_a_tiempo: null,
    });
  });

  it('devuelve null para activación si Identity responde con error o datos inválidos', async () => {
    const { pg, servicio } = crear();
    pg.query.mockResolvedValueOnce([]).mockResolvedValueOnce([]).mockResolvedValueOnce([]).mockResolvedValueOnce([]);
    pg.queryOne.mockResolvedValueOnce({ n: 0 });
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, json: vi.fn() }));

    await expect(servicio.kpis(periodo)).resolves.toMatchObject({ activacion_primeros_15_dias: null });

    const segunda = crear();
    segunda.pg.query
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([]);
    segunda.pg.queryOne.mockResolvedValueOnce({ n: 0 });
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: vi.fn().mockResolvedValue({ vendedores: [] }) }));
    await expect(segunda.servicio.kpis(periodo)).resolves.toMatchObject({ activacion_primeros_15_dias: null });
  });

  it('tolera una excepción de Identity y conserva los demás KPIs', async () => {
    const { pg, servicio } = crear();
    pg.query.mockResolvedValueOnce(metricasBase()).mockResolvedValueOnce([]).mockResolvedValueOnce([]).mockResolvedValueOnce([]);
    pg.queryOne.mockResolvedValueOnce({ n: 2 });
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('identity offline')));

    await expect(servicio.kpis(periodo)).resolves.toMatchObject({
      gmv_cents: 123456,
      vendedores_activos: 2,
      activacion_primeros_15_dias: null,
    });
  });

  it('redondea el embudo y devuelve null cuando falta el denominador o no hay checkouts', () => {
    const { servicio } = crear();
    const mapa = new Map([
      [`${periodo}-01|carritos_iniciados`, { monto: 0, cantidad: 6 }],
      [`${periodo}-01|checkouts_completados`, { monto: 0, cantidad: 1 }],
    ]);
    expect(servicio.conversionEmbudo(mapa, `${periodo}-01`)).toBe(0.1667);
    expect(servicio.conversionEmbudo(new Map(), `${periodo}-01`)).toBeNull();
    expect(
      servicio.conversionEmbudo(
        new Map([[`${periodo}-01|carritos_iniciados`, { monto: 0, cantidad: 2 }]]),
        `${periodo}-01`,
      ),
    ).toBeNull();
  });

  it('calcula liquidación a tiempo y devuelve null si no hay cortes esperados', () => {
    const { servicio } = crear();
    expect(
      servicio.liquidacionATiempo(
        new Map([
          [`${periodo}-01|liquidaciones_a_tiempo`, { monto: 0, cantidad: 2 }],
          [`${periodo}-01|cortes_esperados`, { monto: 0, cantidad: 3 }],
        ]),
        `${periodo}-01`,
      ),
    ).toBe(0.6667);
    expect(servicio.liquidacionATiempo(new Map(), `${periodo}-01`)).toBeNull();
  });

  it('lee el GMV real del mes y distingue ausencia de datos', async () => {
    const { pg, servicio } = crear();
    pg.query.mockResolvedValueOnce([{ periodo: '2026-09-01', tipo: 'gmv', monto_cents: 9000, cantidad: 0 }]);
    await expect(servicio.leerGMVReal(periodo)).resolves.toBe(9000);

    pg.query.mockResolvedValueOnce([]);
    await expect(servicio.leerGMVReal(periodo)).resolves.toBeNull();
  });

  it('resume la actividad fiscal de un vendedor y devuelve null sin ventas', async () => {
    const { pg, servicio } = crear();
    pg.queryOne
      .mockResolvedValueOnce({
        vendedor_id: 'v-1',
        ventas: 3,
        monto_cents: 50000,
        comision_cents: 6000,
        devoluciones: 1,
      })
      .mockResolvedValueOnce({
        vendedor_id: 'v-1',
        ventas: 0,
        monto_cents: 0,
        comision_cents: 0,
        devoluciones: 0,
      })
      .mockResolvedValueOnce(null);

    await expect(servicio.resumenDelVendedor('v-1', periodo)).resolves.toEqual({
      vendedor_id: 'v-1',
      periodo,
      ventas: 3,
      monto_cents: 50000,
      comision_cents: 6000,
      devoluciones: 1,
    });
    await expect(servicio.resumenDelVendedor('v-1', periodo)).resolves.toBeNull();
    await expect(servicio.resumenDelVendedor('v-1', periodo)).resolves.toBeNull();
    expect(pg.queryOne).toHaveBeenCalledTimes(3);
  });
});
