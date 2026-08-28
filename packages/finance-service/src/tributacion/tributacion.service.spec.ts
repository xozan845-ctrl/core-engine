import { TributacionService } from './tributacion.service';

describe('TributacionService (Ley 822: declaraciones mensuales de IR e IVA)', () => {
  const crearServicio = () => {
    const fake = {
      pg: { query: jest.fn(), queryOne: jest.fn(), transaccion: jest.fn() },
      outbox: { insertarEnTransaccion: jest.fn(), insertar: jest.fn() },
      contabilidad: { ingresosDelPeriodo: jest.fn(), gastosDelPeriodo: jest.fn() },
      logger: { info: jest.fn(), warn: jest.fn(), error: jest.fn() },
    };
    return { servicio: new TributacionService(fake.pg as never, fake.outbox as never, fake.contabilidad as never), fake };
  };

  it('el periodo mensual cierra el ultimo dia del mes', () => {
    const { servicio } = crearServicio();
    const periodo = servicio.periodoDelMes(new Date(2026, 7, 10)); // 10 agosto 2026
    expect(periodo.inicio).toBe('2026-08-01');
    expect(periodo.fin).toBe('2026-08-31');
  });

  it('febrero no bisiesto: el periodo cierra el dia 28', () => {
    const { servicio } = crearServicio();
    const periodo = servicio.periodoDelMes(new Date(2026, 1, 15));
    expect(periodo.inicio).toBe('2026-02-01');
    expect(periodo.fin).toBe('2026-02-28');
  });

  it('la declaracion del dia 1 corresponde al mes anterior', () => {
    const { servicio } = crearServicio();
    const periodo = servicio.mesAnterior(new Date(2026, 7, 1)); // 1 agosto 2026
    expect(periodo.inicio).toBe('2026-07-01');
    expect(periodo.fin).toBe('2026-07-31');
  });

  it('genera IVA (15 %) e IR (30 %) sobre la base contable del periodo', async () => {
    const { servicio, fake } = crearServicio();
    fake.contabilidad.ingresosDelPeriodo.mockResolvedValue(2700000); // C$ 27 000
    fake.contabilidad.gastosDelPeriodo.mockResolvedValue(200000); // C$ 2 000
    fake.pg.queryOne
      .mockResolvedValueOnce({
        codigo_pais: 'NI',
        nombre: 'Nicaragua',
        moneda: 'C$',
        simbolo_moneda: 'C$',
        tasa_iva: '0.1500',
        tasa_ir: '0.3000',
        periodicidad_declaracion: 'MENSUAL',
        leyes: { iva: 'Ley 822' },
        activa: true,
      })
      .mockResolvedValue(null); // ninguna declaracion existente

    const declaracionCreada: Record<string, Record<string, unknown>> = {};
    const cliente = {
      query: jest.fn(async (sql: string, params: unknown[]) => {
        if (sql.includes('INSERT INTO finance.declaraciones')) {
          const fila = {
            id: `dec-${params[1]}`,
            jurisdiccion: String(params[0]),
            tipo: String(params[1]),
            periodo_inicio: String(params[2]),
            periodo_fin: String(params[3]),
            base_gravada_cents: Number(params[4] ?? 0),
            iva_debitado_cents: Number(params[5] ?? 0),
            iva_credito_cents: Number(params[6] ?? 0),
            iva_a_pagar_cents: Number(params[7] ?? 0),
            renta_bruta_cents: Number(params[8] ?? 0),
            renta_gravable_cents: Number(params[9] ?? 0),
            ir_a_pagar_cents: Number(params[10] ?? 0),
            cuota_cents: Number(params[11] ?? 0),
            estado: 'GENERADA',
            detalle: null,
            generada_en: '2026-08-01T00:10:00.000Z',
            presentada_en: null,
            creado_en: '2026-08-01T00:10:00.000Z',
          };
          declaracionCreada[fila.tipo] = fila;
          return { rows: [fila] };
        }
        return { rows: [] };
      }),
    };
    fake.pg.transaccion.mockImplementation(async (fn: (c: unknown) => unknown) => fn(cliente));

    const declaraciones = await servicio.generarDeclaracionesDelMesAnterior(new Date(2026, 7, 1));

    expect(declaraciones).toHaveLength(3); // IVA, IR y CUOTA_FIJA informativa
    expect(declaracionCreada.IVA.iva_a_pagar_cents).toBe(405000); // 2 700 000 x 15 %
    expect(declaracionCreada.IVA.base_gravada_cents).toBe(2700000);
    expect(declaracionCreada.IR.renta_gravable_cents).toBe(2500000); // ingresos - gastos
    expect(declaracionCreada.IR.ir_a_pagar_cents).toBe(750000); // 2 500 000 x 30 %
    expect(declaracionCreada.CUOTA_FIJA.cuota_cents).toBe(0); // la determina la DGI
    expect(fake.outbox.insertarEnTransaccion).toHaveBeenCalledTimes(3); // declaracion.generada x3
  });

  it('sin actividad declara IVA/IR en cero (obligacion legal) y omite la cuota fija', async () => {
    const { servicio, fake } = crearServicio();
    fake.contabilidad.ingresosDelPeriodo.mockResolvedValue(0);
    fake.contabilidad.gastosDelPeriodo.mockResolvedValue(0);
    fake.pg.queryOne.mockResolvedValueOnce({
      codigo_pais: 'NI',
      nombre: 'Nicaragua',
      moneda: 'C$',
      simbolo_moneda: 'C$',
      tasa_iva: '0.1500',
      tasa_ir: '0.3000',
      periodicidad_declaracion: 'MENSUAL',
      leyes: {},
      activa: true,
    });
    const declaracionCreada: Record<string, Record<string, unknown>> = {};
    const cliente = {
      query: jest.fn(async (sql: string, params: unknown[]) => {
        if (sql.includes('INSERT INTO finance.declaraciones')) {
          const fila = {
            id: `dec-${params[1]}`,
            tipo: String(params[1]),
            base_gravada_cents: Number(params[4] ?? 0),
            iva_a_pagar_cents: Number(params[7] ?? 0),
            renta_gravable_cents: Number(params[9] ?? 0),
            ir_a_pagar_cents: Number(params[10] ?? 0),
            cuota_cents: Number(params[11] ?? 0),
          };
          declaracionCreada[fila.tipo] = fila;
          return { rows: [fila] };
        }
        return { rows: [] };
      }),
    };
    fake.pg.transaccion.mockImplementation(async (fn: (c: unknown) => unknown) => fn(cliente));

    const declaraciones = await servicio.generarDeclaracionesDelMesAnterior(new Date(2026, 7, 1));
    expect(declaraciones.map((d) => d.tipo).sort()).toEqual(['IR', 'IVA']);
    expect(declaracionCreada.IVA.iva_a_pagar_cents).toBe(0);
    expect(declaracionCreada.IR.ir_a_pagar_cents).toBe(0);
    expect(declaracionCreada.CUOTA_FIJA).toBeUndefined();
  });
});