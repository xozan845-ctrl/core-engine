import { ContabilidadService } from './contabilidad.service';
import type { ParametrosAsiento } from './contabilidad.service';
import { DomainError, EVENTOS } from '@core/shared';

function crear() {
  const client = { query: vi.fn() };
  const pg = {
    query: vi.fn(),
    queryOne: vi.fn(),
    transaccion: vi.fn(async (fn: (c: unknown) => Promise<unknown>) => fn(client)),
  };
  const outbox = { insertarEnTransaccion: vi.fn() };
  return {
    servicio: new ContabilidadService(pg as never, outbox as never),
    pg,
    client,
    outbox,
  };
}

const asiento = {
  id: 'a1',
  fecha: '2026-08-01T00:00:00.000Z',
  concepto: 'Venta',
  tipo: 'INGRESO',
  referencia_tipo: 'orden',
  referencia_id: 'o1',
  moneda: 'C$',
  estado: 'REGISTRADO',
  creado_por: 'u1',
  creado_en: '2026-08-01T00:00:00.000Z',
};

const params: ParametrosAsiento = {
  concepto: 'Venta',
  tipo: 'INGRESO',
  detalles: [
    { cuenta_codigo: '1.1', debe_cents: 100, haber_cents: 0 },
    { cuenta_codigo: '4.1', debe_cents: 0, haber_cents: 100, concepto: 'ingreso', orden: 5 },
  ],
};

describe('ContabilidadService (mutantes dirigidos)', () => {
  describe('validarPartidaDoble', () => {
    it('rechaza menos de dos movimientos', () => {
      const { servicio } = crear();
      expect(() => servicio.validarPartidaDoble([])).toThrow(
        /al menos dos movimientos/,
      );
      expect(() =>
        servicio.validarPartidaDoble([{ cuenta_codigo: '1', debe_cents: 1, haber_cents: 0 }]),
      ).toThrow(DomainError);
      try {
        servicio.validarPartidaDoble([]);
      } catch (error) {
        expect(error).toMatchObject({
          codigo: 'PARTIDA_DOBLE_INVALIDA',
          message: 'Un asiento requiere al menos dos movimientos.',
        });
      }
    });

    it('rechaza asientos descuadrados o sin importes', () => {
      const { servicio } = crear();
      expect(() =>
        servicio.validarPartidaDoble([
          { cuenta_codigo: '1', debe_cents: 100, haber_cents: 0 },
          { cuenta_codigo: '2', debe_cents: 0, haber_cents: 99 },
        ]),
      ).toThrow(/no cuadra: debe 100 no es igual a haber 99/);
      let error: unknown;
      try {
        servicio.validarPartidaDoble([
          { cuenta_codigo: '1', debe_cents: 0, haber_cents: 0 },
          { cuenta_codigo: '2', debe_cents: 0, haber_cents: 0 },
        ]);
      } catch (e) {
        error = e;
      }
      expect(error).toMatchObject({
        codigo: 'PARTIDA_DOBLE_DESCUADRADA',
        message: 'El asiento no cuadra: debe 0 no es igual a haber 0.',
      });
    });

    it('acepta una partida doble válida', () => {
      const { servicio } = crear();
      expect(() => servicio.validarPartidaDoble(params.detalles)).not.toThrow();
    });
  });

  it('registra asiento, detalles y outbox en la transacción', async () => {
    const { servicio, client, outbox } = crear();
    client.query
      .mockResolvedValueOnce({ rows: [asiento] })
      .mockResolvedValueOnce({ rows: [] })
      .mockResolvedValueOnce({ rows: [] });

    const resultado = await servicio.registrarEnTransaccion(client as never, params);

    expect(client.query).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('INSERT INTO finance.asientos'),
      [null, 'Venta', 'INGRESO', null, null, 'C$', null],
    );
    expect(client.query).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('INSERT INTO finance.asiento_detalles'),
      ['a1', '1.1', 100, 0, null, 0],
    );
    expect(client.query).toHaveBeenNthCalledWith(
      3,
      expect.stringContaining('INSERT INTO finance.asiento_detalles'),
      ['a1', '4.1', 0, 100, 'ingreso', 5],
    );
    expect(outbox.insertarEnTransaccion).toHaveBeenCalledWith(
      client,
      EVENTOS.ASIENTO_REGISTRADO,
      expect.objectContaining({
        asiento_id: 'a1',
        debe_cents: 100,
        haber_cents: 100,
        moneda: 'C$',
      }),
    );
    expect(resultado).toMatchObject({
      id: 'a1',
      debe_cents: 100,
      haber_cents: 100,
      detalles: params.detalles,
    });
  });

  it('registrarEnTransaccion valida la partida antes de insertar', async () => {
    const { servicio, client } = crear();
    await expect(
      servicio.registrarEnTransaccion(client as never, { ...params, detalles: [] }),
    ).rejects.toMatchObject({ codigo: 'PARTIDA_DOBLE_INVALIDA' });
    expect(client.query).not.toHaveBeenCalled();
  });

  it('registrar envuelve el asiento en una transacción', async () => {
    const { servicio, pg, client } = crear();
    client.query.mockResolvedValueOnce({ rows: [asiento] });
    pg.queryOne.mockResolvedValue(null);

    const resultado = await servicio.registrar(params);

    expect(pg.transaccion).toHaveBeenCalledTimes(1);
    expect(resultado.id).toBe('a1');
  });

  it('anular devuelve la fila, null si no existe y null si ya no está registrado', async () => {
    const { servicio, pg } = crear();

    pg.queryOne.mockResolvedValueOnce({ ...asiento, estado: 'ANULADO' });
    await expect(servicio.anular('a1')).resolves.toMatchObject({ estado: 'ANULADO' });

    pg.queryOne.mockReset();
    pg.queryOne.mockResolvedValueOnce(null).mockResolvedValueOnce(null);
    await expect(servicio.anular('a2')).resolves.toBeNull();
    expect(pg.queryOne).toHaveBeenCalledTimes(2);

    pg.queryOne.mockReset();
    pg.queryOne.mockResolvedValueOnce(null).mockResolvedValueOnce({ id: 'a1' });
    await expect(servicio.anular('a3')).resolves.toBeNull();
    expect(pg.queryOne).toHaveBeenCalledTimes(2);

    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining("estado = 'ANULADO'"),
      ['a3'],
    );
  });

  it('libroDiario aplica filtros y acota el límite entre 1 y 500', async () => {
    const { servicio, pg } = crear();
    pg.query.mockResolvedValue([asiento]);

    await servicio.libroDiario('2026-08-01', '2026-08-31', 1000);
    await servicio.libroDiario(undefined, undefined, 0);

    expect(pg.query).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('FROM finance.asientos_vista'),
      ['2026-08-01', '2026-08-31', 500],
    );
    expect(pg.query).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('LIMIT $3'),
      [null, null, 1],
    );
  });

  it('libroMayor rechaza una cuenta inexistente', async () => {
    const { servicio, pg } = crear();
    pg.queryOne.mockResolvedValueOnce(null);

    await expect(servicio.libroMayor('9.9')).rejects.toMatchObject({
      codigo: 'CUENTA_INEXISTENTE',
      message: 'La cuenta 9.9 no existe en el plan.',
    });
  });

  it('libroMayor calcula saldo inicial y final para cuenta deudora', async () => {
    const { servicio, pg } = crear();
    pg.queryOne
      .mockResolvedValueOnce({
        codigo: '1.1',
        nombre: 'Caja',
        tipo: 'ACTIVO',
        naturaleza: 'DEUDORA',
        nivel: 2,
        estado: 'activa',
      })
      .mockResolvedValueOnce({ saldo: 100 });
    pg.query.mockResolvedValue([
      { movimiento: { debe_cents: 50, haber_cents: 20 } },
    ]);

    const resultado = await servicio.libroMayor('1.1', undefined, '2026-08-15');

    expect(pg.queryOne).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('FROM finance.plan_cuentas WHERE codigo = $1'),
      ['1.1'],
    );
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('SUM(debe_cents) - SUM(haber_cents)'),
      ['1.1', '2026-08-01'],
    );
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('jsonb_array_elements'),
      ['1.1', '2026-08-01'],
    );
    expect(resultado).toMatchObject({
      saldo_inicial_cents: 100,
      saldo_final_cents: 130,
    });
  });

  it('libroMayor invierte el saldo para cuenta acreedora', async () => {
    const { servicio, pg } = crear();
    pg.queryOne
      .mockResolvedValueOnce({
        codigo: '2.1',
        nombre: 'Proveedores',
        tipo: 'PASIVO',
        naturaleza: 'ACREEDORA',
        nivel: 2,
        estado: 'activa',
      })
      .mockResolvedValueOnce({ saldo: 100 });
    pg.query.mockResolvedValue([
      { movimiento: { debe_cents: 50, haber_cents: 20 } },
    ]);

    const resultado = await servicio.libroMayor('2.1', undefined, '2026-08-15');

    expect(resultado.saldo_inicial_cents).toBe(-100);
    expect(resultado.saldo_final_cents).toBe(-130);
  });

  it('libroMayor usa saldo inicial cero si no hay periodos previos', async () => {
    const { servicio, pg } = crear();
    pg.queryOne
      .mockResolvedValueOnce({
        codigo: '1.1',
        nombre: 'Caja',
        tipo: 'ACTIVO',
        naturaleza: 'DEUDORA',
        nivel: 2,
        estado: 'activa',
      })
      .mockResolvedValueOnce(null);
    pg.query.mockResolvedValue([]);

    const resultado = await servicio.libroMayor('1.1', undefined, '2026-08-15');

    expect(resultado.saldo_inicial_cents).toBe(0);
    expect(resultado.saldo_final_cents).toBe(0);
  });

  it('ingresos y gastos usan el tipo de cuenta y devuelven cero si no hay fila', async () => {
    const { servicio, pg } = crear();
    pg.queryOne.mockResolvedValueOnce({ total: 250 }).mockResolvedValueOnce(null);

    await expect(servicio.ingresosDelPeriodo('2026-08-01', '2026-08-31')).resolves.toBe(250);
    await expect(servicio.gastosDelPeriodo('2026-08-01', '2026-08-31')).resolves.toBe(0);

    expect(pg.queryOne).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining("c.tipo = 'INGRESO'"),
      ['2026-08-01', '2026-08-31'],
    );
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining("c.tipo IN ('GASTO', 'COSTO')"),
      ['2026-08-01', '2026-08-31'],
    );

    pg.queryOne.mockReset();
    pg.queryOne.mockResolvedValue(null);
    await expect(servicio.ingresosDelPeriodo('2026-08-01', '2026-08-31')).resolves.toBe(0);
  });

  it('planDeCuentas ordena por código', async () => {
    const { servicio, pg } = crear();
    pg.query.mockResolvedValue([]);
    await servicio.planDeCuentas();
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('ORDER BY codigo'));
  });

  it('crearCuenta valida el código, el nivel por defecto y la duplicada', async () => {
    const { servicio, pg } = crear();
    pg.queryOne
      .mockResolvedValueOnce({ codigo: '1.1.3' })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ codigo: '12.10' });

    await expect(
      servicio.crearCuenta({
        codigo: '1.1.3',
        nombre: 'Caja',
        tipo: 'ACTIVO',
        naturaleza: 'DEUDORA',
      }),
    ).resolves.toMatchObject({ codigo: '1.1.3' });
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('INSERT INTO finance.plan_cuentas'),
      ['1.1.3', 'Caja', 'ACTIVO', 'DEUDORA', 3],
    );

    await expect(
      servicio.crearCuenta({
        codigo: '1.1.4',
        nombre: 'Caja 2',
        tipo: 'ACTIVO',
        naturaleza: 'DEUDORA',
      }),
    ).rejects.toMatchObject({
      codigo: 'CUENTA_DUPLICADA',
      message: 'La cuenta 1.1.4 ya existe.',
    });

    await expect(
      servicio.crearCuenta({
        codigo: '12.10',
        nombre: 'Caja 3',
        tipo: 'ACTIVO',
        naturaleza: 'DEUDORA',
      }),
    ).resolves.toMatchObject({ codigo: '12.10' });

    for (const codigo of ['', '1..2', 'a', '.1', '1.']) {
      await expect(
        servicio.crearCuenta({
          codigo,
          nombre: 'X',
          tipo: 'ACTIVO',
          naturaleza: 'DEUDORA',
        }),
      ).rejects.toMatchObject({ codigo: 'CODIGO_CUENTA_INVALIDO' });
    }
  });

  it('estadoCuenta, libroVentas y libroCompras pasan los parámetros esperados', async () => {
    const { servicio, pg } = crear();
    pg.queryOne.mockResolvedValue({ codigo: '1.1', estado: 'inactiva' });
    pg.query.mockResolvedValue([]);

    await servicio.estadoCuenta('1.1', 'inactiva');
    await servicio.libroVentas('2026-08-01', '2026-08-31');
    await servicio.libroCompras();

    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining('UPDATE finance.plan_cuentas SET estado = $2'),
      ['1.1', 'inactiva'],
    );
    expect(pg.query).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining("WHERE estado = 'EMITIDO'"),
      ['2026-08-01', '2026-08-31'],
    );
    expect(pg.query).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining("0::int AS comprobantes"),
      [null, null],
    );
  });

  it('formatear usa Money con dos decimales', () => {
    const { servicio } = crear();
    expect(servicio.formatear(0)).toBe('0.00');
    expect(servicio.formatear(115_000)).toBe('1150.00');
  });
});
