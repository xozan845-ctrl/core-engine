import { ContabilidadService } from './contabilidad.service';

describe('ContabilidadService (partida doble, bitacora append-only)', () => {
  const crearServicio = () => {
    const fake = {
      pg: { query: jest.fn(), queryOne: jest.fn(), transaccion: jest.fn() },
      outbox: { insertarEnTransaccion: jest.fn() },
      logger: { info: jest.fn(), warn: jest.fn(), error: jest.fn() },
    };
    return { servicio: new ContabilidadService(fake.pg as never, fake.outbox as never), fake };
  };

  it('valida la ecuacion de partida doble (debe == haber)', () => {
    const { servicio } = crearServicio();
    expect(() =>
      servicio.validarPartidaDoble([
        { cuenta_codigo: '1.1.1', debe_cents: 100, haber_cents: 0 },
        { cuenta_codigo: '2.1.2', debe_cents: 0, haber_cents: 100 },
      ]),
    ).not.toThrow();
    expect(() =>
      servicio.validarPartidaDoble([
        { cuenta_codigo: '1.1.1', debe_cents: 100, haber_cents: 0 },
        { cuenta_codigo: '2.1.2', debe_cents: 0, haber_cents: 90 },
      ]),
    ).toThrow('no cuadra');
  });

  it('rechaza asientos de un solo lado o sin movimientos', () => {
    const { servicio } = crearServicio();
    expect(() =>
      servicio.validarPartidaDoble([{ cuenta_codigo: '1.1.1', debe_cents: 100, haber_cents: 0 }]),
    ).toThrow('requiere al menos dos movimientos');
    expect(() =>
      servicio.validarPartidaDoble([
        { cuenta_codigo: '1.1.1', debe_cents: 0, haber_cents: 0 },
        { cuenta_codigo: '2.1.1', debe_cents: 0, haber_cents: 0 },
      ]),
    ).toThrow('no cuadra');
  });

  it('registra el asiento con el outbox dentro de la misma transaccion (ADR-03)', async () => {
    const { servicio, fake } = crearServicio();
    const filaAsiento = {
      id: 'as-1',
      fecha: '2026-08-09T00:00:00.000Z',
      concepto: 'Pago de la orden abc',
      tipo: 'INGRESO',
      referencia_tipo: 'order',
      referencia_id: 'abc',
      moneda: 'C$',
      estado: 'REGISTRADO',
      creado_por: 'sistema',
      creado_en: '2026-08-09T00:00:00.000Z',
    };
    const cliente = {
      query: jest.fn().mockResolvedValue({ rows: [filaAsiento] }),
    };
    fake.pg.transaccion.mockImplementation(async (fn: (c: unknown) => unknown) => fn(cliente));

    const asiento = await servicio.registrar({
      concepto: 'Pago de la orden abc',
      tipo: 'INGRESO',
      referencia_tipo: 'order',
      referencia_id: 'abc',
      creado_por: 'sistema',
      detalles: [
        { cuenta_codigo: '1.1.1', debe_cents: 1000, haber_cents: 0 },
        { cuenta_codigo: '2.1.2', debe_cents: 0, haber_cents: 1000 },
      ],
    });

    expect(asiento.debe_cents).toBe(1000);
    expect(asiento.haber_cents).toBe(1000);
    expect(fake.outbox.insertarEnTransaccion).toHaveBeenCalledTimes(1);
    expect(fake.outbox.insertarEnTransaccion.mock.calls[0][0]).toBe(cliente);
  });

  it('la anulacion solo marca el estado (append-only, auditoria DGI)', async () => {
    const { servicio, fake } = crearServicio();
    const filaAsiento = {
      id: 'as-2',
      fecha: '2026-08-09T00:00:00.000Z',
      concepto: 'Ajuste de devolucion',
      tipo: 'AJUSTE',
      referencia_tipo: 'order',
      referencia_id: 'xyz',
      moneda: 'C$',
      estado: 'ANULADO',
      creado_por: 'sistema',
      creado_en: '2026-08-09T00:00:00.000Z',
    };
    fake.pg.queryOne.mockResolvedValue(filaAsiento);
    const asiento = await servicio.anular('as-2');
    expect(asiento?.estado).toBe('ANULADO');
    const sql = String(fake.pg.queryOne.mock.calls[0][0]);
    expect(sql).toContain("SET estado = 'ANULADO'");
  });

  it('forma cuentas jerarquicas validas y rechaza codigos mal formados', async () => {
    const { servicio, fake } = crearServicio();
    fake.pg.queryOne.mockResolvedValue({
      codigo: '1.1.1',
      nombre: 'Caja',
      tipo: 'ACTIVO',
      naturaleza: 'DEUDORA',
      nivel: 3,
      estado: 'activa',
    });
    const cuenta = await servicio.crearCuenta({
      codigo: '1.1.1',
      nombre: 'Caja',
      tipo: 'ACTIVO',
      naturaleza: 'DEUDORA',
    });
    expect(cuenta.codigo).toBe('1.1.1');
    await expect(
      servicio.crearCuenta({ codigo: '11a', nombre: 'x', tipo: 'ACTIVO', naturaleza: 'DEUDORA' }),
    ).rejects.toThrow('codigo de cuenta debe ser jerarquico');
  });
});