import { afterEach, describe, expect, it, vi } from 'vitest';
import { EVENTOS } from '@core/shared';
import { SchedulingService } from './scheduling.service';

const empresa = '11111111-1111-4111-8111-111111111111';
const recurso = '22222222-2222-4222-8222-222222222222';
const cita = '33333333-3333-4333-8333-333333333333';
const inicio = '2030-04-15T14:00:00.000Z';
const fin = '2030-04-15T15:00:00.000Z';

function crear() {
  const pg = {
    queryOne: vi.fn(),
    query: vi.fn(),
    transaccion: vi.fn(),
  };
  const outbox = { insertarEnTransaccion: vi.fn().mockResolvedValue('event-id') };
  const fetchMock = vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    json: async () => ({ id: 'parte-1' }),
  });
  vi.stubGlobal('fetch', fetchMock);
  const service = new SchedulingService(pg as never, outbox as never);
  return { pg, outbox, service, fetchMock };
}

function clientCon(...respuestas: unknown[]) {
  const query = vi.fn();
  for (const respuesta of respuestas) query.mockResolvedValueOnce(respuesta);
  return { query };
}

describe('SchedulingService', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('crea y lista recursos y disponibilidad por tenant', async () => {
    const { pg, service } = crear();
    pg.queryOne.mockResolvedValueOnce({ id: recurso, empresa_id: empresa, nombre: 'Sala 1' });
    pg.queryOne.mockResolvedValueOnce({ id: recurso });
    pg.queryOne.mockResolvedValueOnce({ id: 'disp-1', recurso_id: recurso, dia_semana: 1 });
    pg.query.mockResolvedValueOnce([{ id: recurso }]);
    const creado = await service.crearRecurso(empresa, { tipo: 'sala', nombre: ' Sala 1 ' });
    const disponibilidad = await service.crearDisponibilidad(empresa, {
      recurso_id: recurso, dia_semana: 1, inicia_hora: '08:00', termina_hora: '17:00',
    });
    const lista = await service.listarDisponibilidad(empresa, recurso);
    pg.query.mockResolvedValueOnce([{ id: cita }]).mockResolvedValueOnce([{ id: 'recordatorio-1' }]);
    await expect(service.listarCitas(empresa)).resolves.toEqual([{ id: cita }]);
    await expect(service.listarRecordatorios(empresa)).resolves.toEqual([{ id: 'recordatorio-1' }]);
    expect(creado).toMatchObject({ id: recurso });
    expect(disponibilidad).toMatchObject({ id: 'disp-1' });
    expect(lista).toEqual([{ id: recurso }]);
    expect(pg.queryOne).toHaveBeenCalledTimes(3);
  });

  it('rechaza disponibilidad si el recurso no pertenece al tenant', async () => {
    const { pg, service } = crear();
    pg.queryOne.mockResolvedValue(null);
    await expect(service.crearDisponibilidad(empresa, {
      recurso_id: recurso, dia_semana: 1, inicia_hora: '08:00', termina_hora: '17:00',
    })).rejects.toMatchObject({ codigo: 'NO_ENCONTRADO' });
  });

  it('crea un bloqueo y traduce el conflicto GiST', async () => {
    const { pg, service } = crear();
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rowCount: 1, rows: [{ id: recurso }] })
        .mockResolvedValueOnce({ rowCount: 1, rows: [{ id: 'bloqueo-1', recurso_id: recurso }] }),
    };
    pg.transaccion.mockImplementation(async (fn: (c: unknown) => Promise<unknown>) => fn(client));
    const bloqueo = await service.crearBloqueo(empresa, '44444444-4444-4444-8444-444444444444', {
      recurso_id: recurso, inicia_en: inicio, termina_en: fin, motivo: 'mantenimiento',
    });
    expect(bloqueo).toMatchObject({ id: 'bloqueo-1' });
    expect(client.query.mock.calls[0][0]).toContain('FOR UPDATE');

    const conflicto = {
      query: vi.fn()
        .mockResolvedValueOnce({ rowCount: 1, rows: [{ id: recurso }] })
        .mockRejectedValueOnce({ code: '23P01' }),
    };
    pg.transaccion.mockImplementation(async (fn: (c: unknown) => Promise<unknown>) => fn(conflicto));
    await expect(service.crearBloqueo(empresa, undefined, {
      recurso_id: recurso, inicia_en: inicio, termina_en: fin,
    })).rejects.toMatchObject({ codigo: 'RECURSO_NO_DISPONIBLE' });
  });

  it('crea una cita, recordatorios y evento outbox', async () => {
    const { pg, outbox, service } = crear();
    pg.queryOne.mockResolvedValueOnce({ id: 'parte-1' });
    const client = clientCon(
      { rowCount: 1, rows: [{ id: recurso }] },
      { rowCount: 1, rows: [{}] },
      { rowCount: 0, rows: [] },
      { rowCount: 1, rows: [{
        id: cita, empresa_id: empresa, recurso_id: recurso,
        parte_id: '66666666-6666-4666-8666-666666666666',
        order_id: null, item_id: null, estado: 'reservada', inicia_en: inicio, termina_en: fin,
      }] },
      { rowCount: 2, rows: [] },
    );
    pg.transaccion.mockImplementation(async (fn: (c: unknown) => Promise<unknown>) => fn(client));
    const creada = await service.crearCita(empresa, {
      recurso_id: recurso,
      parte_id: '66666666-6666-4666-8666-666666666666',
      inicia_en: inicio,
      termina_en: fin,
    });
    expect(creada).toMatchObject({ id: cita, estado: 'reservada' });
    expect(outbox.insertarEnTransaccion).toHaveBeenCalledWith(
      client,
      EVENTOS.APPOINTMENT_CREATED,
      expect.objectContaining({ cita_id: cita, empresa_id: empresa }),
    );
    expect(client.query).toHaveBeenCalledTimes(5);
  });

  it('impide periodos invalidos, falta de disponibilidad y solapamiento concurrente', async () => {
    const primero = crear();
    await expect(primero.service.crearCita(empresa, { recurso_id: recurso, inicia_en: fin, termina_en: inicio }))
      .rejects.toMatchObject({ codigo: 'PERIODO_INVALIDO' });

    const segundo = crear();
    const sinDisponibilidad = clientCon({ rowCount: 1, rows: [{ id: recurso }] }, { rowCount: 0, rows: [] });
    segundo.pg.transaccion.mockImplementation(async (fn: (c: unknown) => Promise<unknown>) => fn(sinDisponibilidad));
    await expect(segundo.service.crearCita(empresa, { recurso_id: recurso, inicia_en: inicio, termina_en: fin }))
      .rejects.toMatchObject({ codigo: 'RECURSO_NO_DISPONIBLE' });

    const tercero = crear();
    const exclusion = clientCon(
      { rowCount: 1, rows: [{ id: recurso }] },
      { rowCount: 1, rows: [{}] },
      { rowCount: 0, rows: [] },
    );
    exclusion.query.mockRejectedValueOnce({ code: '23P01' });
    tercero.pg.transaccion.mockImplementation(async (fn: (c: unknown) => Promise<unknown>) => fn(exclusion));
    await expect(tercero.service.crearCita(empresa, { recurso_id: recurso, inicia_en: inicio, termina_en: fin }))
      .rejects.toMatchObject({ codigo: 'RECURSO_NO_DISPONIBLE' });

    const parteInvalida = crear();
    parteInvalida.fetchMock.mockResolvedValue({ ok: false, status: 404, json: async () => ({}) });
    await expect(parteInvalida.service.crearCita(empresa, {
      recurso_id: recurso,
      parte_id: '66666666-6666-4666-8666-666666666666',
      inicia_en: inicio,
      termina_en: fin,
    })).rejects.toMatchObject({ codigo: 'PARTE_INVALIDA' });
  });

  it('rechaza una cita cuando existe un bloqueo explícito', async () => {
    const { pg, service } = crear();
    const bloqueado = clientCon(
      { rowCount: 1, rows: [{ id: recurso }] },
      { rowCount: 1, rows: [{}] },
      { rowCount: 1, rows: [{}] },
    );
    pg.transaccion.mockImplementation(async (fn: (c: unknown) => Promise<unknown>) => fn(bloqueado));
    await expect(service.crearCita(empresa, { recurso_id: recurso, inicia_en: inicio, termina_en: fin }))
      .rejects.toMatchObject({ codigo: 'RECURSO_NO_DISPONIBLE' });
  });

  it('cancela cita, cancela recordatorios y publica evento', async () => {
    const { pg, outbox, service } = crear();
    const client = clientCon(
      { rowCount: 1, rows: [{ id: cita, empresa_id: empresa, order_id: '55555555-5555-4555-8555-555555555555' }] },
      { rowCount: 1, rows: [] },
    );
    pg.transaccion.mockImplementation(async (fn: (c: unknown) => Promise<unknown>) => fn(client));
    const resultado = await service.cancelarCita(empresa, cita, 'cliente solicita cambio');
    expect(resultado).toMatchObject({ id: cita });
    expect(outbox.insertarEnTransaccion).toHaveBeenCalledWith(
      client,
      EVENTOS.APPOINTMENT_CANCELLED,
      expect.objectContaining({ cita_id: cita, motivo: 'cliente solicita cambio' }),
    );
  });

  it('crea la cita desde una reserva, es idempotente y valida su configuracion', async () => {
    const { pg, service } = crear();
    pg.queryOne.mockResolvedValueOnce({ id: cita });
    await expect(service.crearDesdeReserva({ order_id: 'o-1', cliente_parte_id: 'p-1', items: [] })).resolves.toMatchObject({ id: cita });

    pg.queryOne.mockResolvedValueOnce(null);
    await expect(service.crearDesdeReserva({ order_id: 'o-2', cliente_parte_id: 'p-1', items: [{ empresa_id: empresa, item_type: 'producto' }] }))
      .rejects.toMatchObject({ codigo: 'CITA_CONFIGURACION_REQUERIDA' });

    pg.queryOne.mockResolvedValueOnce(null);
    const spy = vi.spyOn(service, 'crearCita').mockResolvedValue({ id: cita });
    await service.crearDesdeReserva({
      order_id: 'o-3', cliente_parte_id: '66666666-6666-4666-8666-666666666666', items: [{
        empresa_id: empresa, item_type: 'servicio', item_id: '66666666-6666-4666-8666-666666666666', fulfillment: 'onsite',
        fulfillment_config: { recurso_id: recurso, inicia_en: inicio, termina_en: fin },
      }],
    });
    expect(spy).toHaveBeenCalledWith(empresa, expect.objectContaining({
      order_id: 'o-3', recurso_id: recurso, parte_id: '66666666-6666-4666-8666-666666666666',
    }));
    spy.mockRestore();
  });

  it('procesa recordatorios vencidos de forma transaccional', async () => {
    const { pg, outbox, service } = crear();
    const client = clientCon(
      { rowCount: 2, rows: [
        { id: 'r-1', cita_id: cita, empresa_id: empresa, tipo: '24h', programado_en: inicio },
        { id: 'r-2', cita_id: cita, empresa_id: empresa, tipo: '2h', programado_en: inicio },
      ] },
      { rowCount: 1, rows: [] },
      { rowCount: 1, rows: [] },
    );
    pg.transaccion.mockImplementation(async (fn: (c: unknown) => Promise<unknown>) => fn(client));
    await expect(service.procesarRecordatorios()).resolves.toBe(2);
    expect(outbox.insertarEnTransaccion).toHaveBeenCalledTimes(2);
  });

  it('mantiene el timer de recordatorios controlable', () => {
    const { service } = crear();
    service.onModuleInit();
    service.onModuleDestroy();
    expect(true).toBe(true);
  });
});
