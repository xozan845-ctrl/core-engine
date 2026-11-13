import type { Mock } from 'vitest';
import { ConflictError, DomainError, NotFoundError } from '@core/shared';
import { ContratosService } from './contratos.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const CONTRATO = '22222222-2222-4222-8222-222222222222';
const ORDEN = '33333333-3333-4333-8333-333333333333';
const PARTE = '44444444-4444-4444-8444-444444444444';
const ACTOR = '55555555-5555-4555-8555-555555555555';

const fila = {
  id: CONTRATO,
  empresa_id: EMPRESA,
  order_id: ORDEN,
  parte_id: PARTE,
  item_id: null,
  tipo: 'alquiler' as const,
  titulo: 'Alquiler de equipo',
  descripcion: '',
  estado: 'borrador' as const,
  inicia_en: '2026-12-01',
  termina_en: '2026-12-10',
  deposito_cents: 50000,
  garantia_cents: 20000,
  retenido_cents: 0,
  moneda: 'NIO',
  metadata: {},
  creado_por: ACTOR,
  creado_en: '2026-11-13T00:00:00.000Z',
  actualizado_en: '2026-11-13T00:00:00.000Z',
  finalizado_en: null,
};

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn(), transaccion: vi.fn() };
  pg.transaccion.mockImplementation(async (fn: (client: unknown) => unknown) =>
    fn({ query: vi.fn() }),
  );
  return { servicio: new ContratosService(pg as never), pg };
}

describe('ContratosService (Fase P2.2)', () => {
  it('lista contratos con filtros y serializa montos', async () => {
    const { servicio, pg } = crear();
    (pg.query as Mock).mockResolvedValue([fila]);
    const contratos = await servicio.listar(EMPRESA, {
      estado: 'activo',
      order_id: ORDEN,
      parte_id: PARTE,
      limite: '10',
    });
    expect(contratos[0]).toMatchObject({ deposito: '500.00', garantia: '200.00' });
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('WHERE empresa_id = $1 AND estado = $2 AND order_id = $3 AND parte_id = $4'),
      [EMPRESA, 'activo', ORDEN, PARTE, 10, 0],
    );
  });

  it('crea un contrato y hereda la parte de la orden', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ estado: 'pagada', parte_id: PARTE })
      .mockResolvedValueOnce(fila);
    await expect(
      servicio.crear(EMPRESA, ACTOR, {
        order_id: ORDEN,
        titulo: 'Alquiler de equipo',
        tipo: 'alquiler',
        inicia_en: '2026-12-01',
        termina_en: '2026-12-10',
        deposito: '500.00',
        garantia: '200.00',
      }),
    ).resolves.toMatchObject({ deposito: '500.00', parte_id: PARTE });

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(
      servicio.crear(EMPRESA, ACTOR, { order_id: ORDEN, titulo: 'Contrato' }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it('valida titulo, tipo, fechas, moneda y metadata', async () => {
    const { servicio } = crear();
    await expect(servicio.crear(EMPRESA, ACTOR, { titulo: 'x' })).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, ACTOR, { titulo: 'Contrato', tipo: 'nave' as never }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, ACTOR, {
        titulo: 'Contrato',
        inicia_en: '2026-12-10',
        termina_en: '2026-12-01',
      }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, ACTOR, { titulo: 'Contrato', moneda: 'peso' }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, ACTOR, { titulo: 'Contrato', metadata: [] as never }),
    ).rejects.toBeInstanceOf(DomainError);
  });

  it('activa un contrato y registra el deposito recibido', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(fila);
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [{ ...fila, estado: 'activo' }] })
        .mockResolvedValueOnce({ rows: [] }),
    };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(
      servicio.actualizar(EMPRESA, CONTRATO, { estado: 'activo' }),
    ).resolves.toMatchObject({ estado: 'activo' });
    expect(client.query).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('INSERT INTO orders.contrato_eventos'),
      [EMPRESA, CONTRATO, 'deposito_recibido', 50000, 'Deposito al activar el contrato', ACTOR],
    );
  });

  it('cancela un contrato activo y devuelve el deposito', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, estado: 'activo' });
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [{ ...fila, estado: 'cancelado' }] })
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [] }),
    };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await servicio.actualizar(EMPRESA, CONTRATO, { estado: 'cancelado' });
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.contrato_eventos'),
      [EMPRESA, CONTRATO, 'cancelacion', 0, 'Contrato cancelado', ACTOR],
    );
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.contrato_eventos'),
      [EMPRESA, CONTRATO, 'deposito_devuelto', 50000, 'Devolucion por cancelacion', ACTOR],
    );
  });

  it('rechaza transiciones invalidas y contratos cerrados', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, estado: 'activo' });
    await expect(
      servicio.actualizar(EMPRESA, CONTRATO, { estado: 'borrador' as never }),
    ).rejects.toBeInstanceOf(ConflictError);

    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, estado: 'finalizado' });
    await expect(
      servicio.actualizar(EMPRESA, CONTRATO, { titulo: 'Otro' }),
    ).rejects.toBeInstanceOf(ConflictError);
  });

  it('finaliza un contrato y registra retencion y devolucion', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, estado: 'activo' });
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [{ ...fila, estado: 'finalizado', retenido_cents: 10000 }] })
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [] }),
    };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(
      servicio.finalizar(EMPRESA, CONTRATO, ACTOR, { retenido: '100.00', motivo: 'danos' }),
    ).resolves.toMatchObject({ estado: 'finalizado', retenido: '100.00' });
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.contrato_eventos'),
      [EMPRESA, CONTRATO, 'retencion', 10000, 'danos', ACTOR],
    );
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.contrato_eventos'),
      [EMPRESA, CONTRATO, 'deposito_devuelto', 40000, 'danos', ACTOR],
    );
  });

  it('rechaza finalizar un contrato no activo o retener mas que el deposito', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, estado: 'borrador' });
    await expect(
      servicio.finalizar(EMPRESA, CONTRATO, ACTOR, {}),
    ).rejects.toBeInstanceOf(ConflictError);

    (pg.queryOne as Mock).mockResolvedValueOnce({ ...fila, estado: 'activo' });
    await expect(
      servicio.finalizar(EMPRESA, CONTRATO, ACTOR, { retenido: '900.00' }),
    ).rejects.toBeInstanceOf(DomainError);
  });

  it('lista movimientos y valida el contrato', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(fila);
    (pg.query as Mock).mockResolvedValueOnce([{
      id: 'e-1',
      contrato_id: CONTRATO,
      tipo: 'deposito_recibido',
      monto_cents: 50000,
      motivo: '',
      creado_por: ACTOR,
      creado_en: '2026-11-13T00:00:00.000Z',
    }]);
    await expect(servicio.listarEventos(EMPRESA, CONTRATO)).resolves.toMatchObject([
      { tipo: 'deposito_recibido', monto: '500.00' },
    ]);

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.obtener(EMPRESA, CONTRATO)).rejects.toBeInstanceOf(NotFoundError);
    await expect(servicio.obtener(EMPRESA, 'no-uuid')).rejects.toBeInstanceOf(DomainError);
  });
});
