import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  asignarVendedorATienda,
  capacidadesDeEmpresas,
  desasignarVendedorDeTienda,
  obtenerTienda,
  partePerteneceAEmpresa,
  tiendaDeVendedor,
  tiendasPorEmpresa,
  tiendasPorIds,
  timeoutInternoMs,
  registrarCreditoOrden,
  anularCreditoOrden,
  cotizarItemReserva,
  verificarDisponibilidadReserva,
} from './service-clients';

const fetchMock = vi.fn();

function respuesta(status: number, data?: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => data,
  };
}

describe('service-clients (servicio→servicio)', () => {
  beforeEach(() => {
    fetchMock.mockReset();
    vi.stubGlobal('fetch', fetchMock);
    vi.stubEnv('INTERNAL_API_KEY', 'k');
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it('timeoutInternoMs usa 3000 por defecto y descarta valores inválidos', () => {
    expect(timeoutInternoMs()).toBe(3000);
    vi.stubEnv('INTERNAL_REQUEST_TIMEOUT_MS', '1500');
    expect(timeoutInternoMs()).toBe(1500);
    vi.stubEnv('INTERNAL_REQUEST_TIMEOUT_MS', '0');
    expect(timeoutInternoMs()).toBe(3000);
    vi.stubEnv('INTERNAL_REQUEST_TIMEOUT_MS', 'no-numerico');
    expect(timeoutInternoMs()).toBe(3000);
  });

  it('capacidadesDeEmpresas devuelve [] sin argumentos', async () => {
    await expect(capacidadesDeEmpresas([], ['x'])).resolves.toEqual([]);
    await expect(capacidadesDeEmpresas(['e'], [])).resolves.toEqual([]);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('capacidadesDeEmpresas devuelve las capacidades y manda la clave interna', async () => {
    const capacidades = [{ empresa_id: 'e', capacidad_codigo: 'x', activo: true }];
    fetchMock.mockResolvedValue(respuesta(200, { capacidades }));
    await expect(capacidadesDeEmpresas(['e'], ['x'])).resolves.toEqual(capacidades);
    expect(fetchMock).toHaveBeenCalledWith(
      'http://identity-service:3001/internal/empresas/capacidades',
      expect.objectContaining({
        method: 'POST',
        headers: expect.objectContaining({ 'x-internal-key': 'k' }),
        signal: expect.any(AbortSignal),
      }),
    );
  });

  it('capacidadesDeEmpresas devuelve null si identity falla', async () => {
    fetchMock.mockRejectedValue(new Error('caido'));
    await expect(capacidadesDeEmpresas(['e'], ['x'])).resolves.toBeNull();
    fetchMock.mockResolvedValue(respuesta(500));
    await expect(capacidadesDeEmpresas(['e'], ['x'])).resolves.toBeNull();
  });

  it('partePerteneceAEmpresa distingue 404, 200 y caida', async () => {
    fetchMock.mockResolvedValueOnce(respuesta(404));
    await expect(partePerteneceAEmpresa('e', 'p')).resolves.toBe(false);
    fetchMock.mockResolvedValueOnce(respuesta(200, { id: 'p' }));
    await expect(partePerteneceAEmpresa('e', 'p')).resolves.toBe(true);
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(partePerteneceAEmpresa('e', 'p')).resolves.toBeNull();
  });

  it('obtenerTienda distingue encontrada, no encontrada y error', async () => {
    fetchMock.mockResolvedValueOnce(
      respuesta(200, { id: 't', nombre: 'T', empresa_id: 'e', vendedor_id: null }),
    );
    await expect(obtenerTienda('t')).resolves.toMatchObject({ estado: 'ok' });
    fetchMock.mockResolvedValueOnce(respuesta(404));
    await expect(obtenerTienda('t')).resolves.toEqual({ estado: 'no_encontrada' });
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(obtenerTienda('t')).resolves.toEqual({ estado: 'error' });
  });

  it('tiendasPorIds devuelve [] sin ids y null si stores falla', async () => {
    await expect(tiendasPorIds([])).resolves.toEqual([]);
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(tiendasPorIds(['t'])).resolves.toBeNull();
  });

  it('tiendasPorIds y tiendasPorEmpresa devuelven tiendas', async () => {
    const tiendas = [{ id: 't', nombre: 'T', empresa_id: 'e', vendedor_id: null }];
    fetchMock.mockResolvedValueOnce(respuesta(200, { tiendas }));
    await expect(tiendasPorIds(['t'])).resolves.toEqual(tiendas);
    fetchMock.mockResolvedValueOnce(respuesta(200, { tiendas }));
    await expect(tiendasPorEmpresa('e')).resolves.toEqual(tiendas);
  });

  it('tiendasPorEmpresa devuelve null si stores falla', async () => {
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(tiendasPorEmpresa('e')).resolves.toBeNull();
  });

  it('tiendaDeVendedor distingue ok, sin tienda, 404 y error', async () => {
    fetchMock.mockResolvedValueOnce(
      respuesta(200, { tienda: { id: 't', nombre: 'T', empresa_id: 'e', vendedor_id: 'v' } }),
    );
    await expect(tiendaDeVendedor('v')).resolves.toMatchObject({ estado: 'ok' });
    fetchMock.mockResolvedValueOnce(respuesta(200, { tienda: null }));
    await expect(tiendaDeVendedor('v')).resolves.toEqual({ estado: 'no_encontrada' });
    fetchMock.mockResolvedValueOnce(respuesta(404));
    await expect(tiendaDeVendedor('v')).resolves.toEqual({ estado: 'no_encontrada' });
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(tiendaDeVendedor('v')).resolves.toEqual({ estado: 'error' });
  });

  it('asignarVendedorATienda distingue ok, conflicto y error', async () => {
    fetchMock.mockResolvedValueOnce(respuesta(200, { ok: true }));
    await expect(asignarVendedorATienda('t', 'u', 10)).resolves.toBe('ok');
    fetchMock.mockResolvedValueOnce(respuesta(409));
    await expect(asignarVendedorATienda('t', 'u', 10)).resolves.toBe('conflicto');
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(asignarVendedorATienda('t', 'u', 10)).resolves.toBe('error');
  });

  it('registrarCreditoOrden distingue ok, rechazo y error', async () => {
    fetchMock.mockResolvedValueOnce(
      respuesta(201, { credito_id: 'c-1', balance_cents: 100, limite_cents: 500 }),
    );
    await expect(
      registrarCreditoOrden({
        empresa_id: 'e-1',
        parte_id: 'p-1',
        order_id: 'o-1',
        monto_cents: 100,
        concepto: 'Pedido',
      }),
    ).resolves.toEqual({ estado: 'ok', credito: { credito_id: 'c-1', balance_cents: 100, limite_cents: 500 } });
    fetchMock.mockResolvedValueOnce(respuesta(409));
    await expect(
      registrarCreditoOrden({ empresa_id: 'e-1', parte_id: 'p-1', order_id: 'o-1', monto_cents: 100, concepto: 'x' }),
    ).resolves.toEqual({ estado: 'rechazado' });
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(
      registrarCreditoOrden({ empresa_id: 'e-1', parte_id: 'p-1', order_id: 'o-1', monto_cents: 100, concepto: 'x' }),
    ).resolves.toEqual({ estado: 'error' });
  });

  it('anularCreditoOrden devuelve true solo con respuesta válida', async () => {
    fetchMock.mockResolvedValueOnce(respuesta(200, { ok: true }));
    await expect(anularCreditoOrden('o-1')).resolves.toBe(true);
    fetchMock.mockResolvedValueOnce(respuesta(500));
    await expect(anularCreditoOrden('o-1')).resolves.toBe(false);
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(anularCreditoOrden('o-1')).resolves.toBe(false);
  });

  it('cotizarItemReserva devuelve la cotización o null', async () => {
    fetchMock.mockResolvedValueOnce(respuesta(200, { total_cents: 90000, dias: 3 }));
    await expect(
      cotizarItemReserva({ empresa_id: 'e-1', item_id: 'i-1', desde: '2026-12-20', hasta: '2026-12-22' }),
    ).resolves.toEqual({ total_cents: 90000, dias: 3 });
    fetchMock.mockResolvedValueOnce(respuesta(500));
    await expect(
      cotizarItemReserva({ empresa_id: 'e-1', item_id: 'i-1', desde: '2026-12-20', hasta: '2026-12-22' }),
    ).resolves.toBeNull();
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(
      cotizarItemReserva({ empresa_id: 'e-1', item_id: 'i-1', desde: '2026-12-20', hasta: '2026-12-22' }),
    ).resolves.toBeNull();
  });

  it('verificarDisponibilidadReserva distingue libre, ocupado y caída', async () => {
    fetchMock.mockResolvedValueOnce(respuesta(201, { disponible: true }));
    await expect(
      verificarDisponibilidadReserva({
        empresa_id: 'e-1',
        recurso_id: 'r-1',
        inicia_en: '2026-12-20T15:00:00.000Z',
        termina_en: '2026-12-22T11:00:00.000Z',
      }),
    ).resolves.toBe(true);
    fetchMock.mockResolvedValueOnce(respuesta(409));
    await expect(
      verificarDisponibilidadReserva({
        empresa_id: 'e-1',
        recurso_id: 'r-1',
        inicia_en: '2026-12-20T15:00:00.000Z',
        termina_en: '2026-12-22T11:00:00.000Z',
      }),
    ).resolves.toBe(false);
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(
      verificarDisponibilidadReserva({
        empresa_id: 'e-1',
        recurso_id: 'r-1',
        inicia_en: '2026-12-20T15:00:00.000Z',
        termina_en: '2026-12-22T11:00:00.000Z',
      }),
    ).resolves.toBeNull();
  });

  it('desasignarVendedorDeTienda devuelve true solo con respuesta válida', async () => {
    fetchMock.mockResolvedValueOnce(respuesta(200, { ok: true }));
    await expect(desasignarVendedorDeTienda('t', 'u')).resolves.toBe(true);
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(desasignarVendedorDeTienda('t', 'u')).resolves.toBe(false);
  });
});
