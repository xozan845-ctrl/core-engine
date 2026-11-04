import { TemaHerenciaService } from './herencia-tema';

describe('TemaHerenciaService (ADR-42)', () => {
  afterEach(() => vi.unstubAllGlobals());

  const respuesta = (plantilla_override: unknown, tema_override: unknown) => ({
    ok: true,
    status: 200,
    json: async () => ({ plantilla_override, tema_override }),
  });

  it('obtiene y normaliza los overrides de empresa y plataforma', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(respuesta('boutique', { color_primario: '#abc' }))
      .mockResolvedValueOnce(respuesta('minimal', { fuente: 'poppins' }));
    vi.stubGlobal('fetch', fetchMock);
    const servicio = new TemaHerenciaService();

    await expect(servicio.obtenerEmpresa('e-1')).resolves.toEqual({
      plantilla: 'boutique',
      tema: { color_primario: '#ABC' },
    });
    await expect(servicio.obtenerPlataforma()).resolves.toEqual({
      plantilla: 'minimal',
      tema: { fuente: 'poppins' },
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('/internal/empresas/e-1/tema'),
      expect.objectContaining({
        headers: { 'x-internal-key': '' },
        signal: expect.any(AbortSignal),
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('/internal/configuracion-sitio/tema'),
      expect.any(Object),
    );
  });

  it('cachea con TTL y no repite la llamada', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(respuesta('clasica', { color_primario: '#111111' }));
    vi.stubGlobal('fetch', fetchMock);
    const servicio = new TemaHerenciaService();

    await servicio.obtenerEmpresa('e-1');
    await servicio.obtenerEmpresa('e-1');

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('degrada a sin herencia si identity falla o no responde', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 500 })));
    await expect(new TemaHerenciaService().obtenerEmpresa('e-1')).resolves.toEqual({
      plantilla: null,
      tema: {},
    });

    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new Error('sin red');
      }),
    );
    await expect(new TemaHerenciaService().obtenerPlataforma()).resolves.toEqual({
      plantilla: null,
      tema: {},
    });
  });
});
