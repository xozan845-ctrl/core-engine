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

  it('desasignarVendedorDeTienda devuelve true solo con respuesta válida', async () => {
    fetchMock.mockResolvedValueOnce(respuesta(200, { ok: true }));
    await expect(desasignarVendedorDeTienda('t', 'u')).resolves.toBe(true);
    fetchMock.mockRejectedValueOnce(new Error('caido'));
    await expect(desasignarVendedorDeTienda('t', 'u')).resolves.toBe(false);
  });
});
