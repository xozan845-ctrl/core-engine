import type { Mock } from 'vitest';
import { BACKGROUND_POR_DEFECTO } from '../media-background/media-background';
import { ConfiguracionSitioService } from './configuracion-sitio.service';

const OBJETO = '11111111-2222-4333-8444-555555555555';
const POSTER = '99999999-8888-4777-8666-555555555555';
const TENANT = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const ACTUALIZADO = '2026-11-04T12:00:00.000Z';

const fila = (extra: Record<string, unknown> = {}) => ({
  ...BACKGROUND_POR_DEFECTO,
  actualizado_en: ACTUALIZADO,
  ...extra,
});

const objeto = (content_type: string, extra: Record<string, unknown> = {}) => ({
  ok: true,
  status: 200,
  json: async () => ({
    id: OBJETO,
    tenant_id: TENANT,
    estado: 'activo',
    publico: true,
    content_type,
    ...extra,
  }),
});

describe('ConfiguracionSitioService (media background de plataforma, ADR-38)', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  const crear = (queryOne: Mock) => new ConfiguracionSitioService({ queryOne } as never);

  it('devuelve los valores por defecto cuando no existe la fila', async () => {
    const queryOne = vi.fn().mockResolvedValue(null);
    const servicio = crear(queryOne);

    await expect(servicio.obtener()).resolves.toEqual({
      ...BACKGROUND_POR_DEFECTO,
      background_url: null,
      background_poster_url: null,
      actualizado_en: null,
    });
    expect(queryOne).toHaveBeenCalledWith(
      expect.stringContaining('FROM identity.configuracion_plataforma WHERE id = 1'),
    );
  });

  it('deriva las URLs del fondo y del poster al leer', async () => {
    const queryOne = vi.fn().mockResolvedValue(
      fila({
        background_objeto_id: OBJETO,
        background_media_type: 'video',
        background_poster_objeto_id: POSTER,
        background_alt: 'Internacional',
        background_overlay: 40,
        background_position: 'top',
        background_activo: true,
      }),
    );
    const servicio = crear(queryOne);

    await expect(servicio.obtener()).resolves.toMatchObject({
      background_media_type: 'video',
      background_url: `/api/v1/storage/public/objetos/${OBJETO}`,
      background_poster_url: `/api/v1/storage/public/objetos/${POSTER}`,
      background_alt: 'Internacional',
      background_overlay: 40,
      background_position: 'top',
      background_activo: true,
      actualizado_en: ACTUALIZADO,
    });
  });

  it('exige PLATFORM_TENANT_ID para validar los assets de plataforma', async () => {
    vi.stubEnv('PLATFORM_TENANT_ID', '');
    const queryOne = vi.fn().mockResolvedValueOnce(null);
    const servicio = crear(queryOne);

    await expect(
      servicio.actualizar({ background_objeto_id: OBJETO }, 'admin-1'),
    ).rejects.toMatchObject({ codigo: 'PLATFORM_TENANT_ID_NO_CONFIGURADO' });
    expect(queryOne).toHaveBeenCalledTimes(1);
  });

  it('valida una imagen, deriva el tipo y persiste los metadatos', async () => {
    vi.stubEnv('PLATFORM_TENANT_ID', TENANT);
    const fetchMock = vi.fn(async () => objeto('image/png'));
    vi.stubGlobal('fetch', fetchMock);
    const queryOne = vi
      .fn()
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(
        fila({
          background_objeto_id: OBJETO,
          background_media_type: 'imagen',
          background_alt: 'Colección',
          background_overlay: 40,
          background_activo: true,
        }),
      );
    const servicio = crear(queryOne);

    const resultado = await servicio.actualizar(
      {
        background_objeto_id: OBJETO,
        background_alt: '  Colección  ',
        background_overlay: 40,
        background_activo: true,
      },
      'admin-1',
    );

    expect(fetchMock).toHaveBeenCalledWith(
      `http://storage-service:3010/internal/storage/objetos/${OBJETO}?tenant_id=${TENANT}`,
      expect.objectContaining({ headers: { 'x-internal-key': '' }, signal: expect.any(AbortSignal) }),
    );
    expect(queryOne).toHaveBeenLastCalledWith(
      expect.stringContaining('ON CONFLICT (id) DO UPDATE SET'),
      [OBJETO, 'imagen', null, 'Colección', 40, 'center', true, 'admin-1'],
    );
    expect(resultado).toMatchObject({
      background_objeto_id: OBJETO,
      background_media_type: 'imagen',
      background_url: `/api/v1/storage/public/objetos/${OBJETO}`,
      background_alt: 'Colección',
      background_activo: true,
    });
  });

  it('valida video y poster, cada uno contra el tenant de plataforma', async () => {
    vi.stubEnv('PLATFORM_TENANT_ID', TENANT);
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(objeto('video/mp4'))
      .mockResolvedValueOnce(objeto('image/webp'));
    vi.stubGlobal('fetch', fetchMock);
    const queryOne = vi
      .fn()
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(
        fila({
          background_objeto_id: OBJETO,
          background_media_type: 'video',
          background_poster_objeto_id: POSTER,
          background_activo: true,
        }),
      );
    const servicio = crear(queryOne);

    const resultado = await servicio.actualizar(
      { background_objeto_id: OBJETO, background_poster_objeto_id: POSTER, background_activo: true },
      'admin-1',
    );

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[0][0]).toContain(`/objetos/${OBJETO}?tenant_id=${TENANT}`);
    expect(fetchMock.mock.calls[1][0]).toContain(`/objetos/${POSTER}?tenant_id=${TENANT}`);
    expect(resultado).toMatchObject({
      background_media_type: 'video',
      background_poster_url: `/api/v1/storage/public/objetos/${POSTER}`,
    });
  });

  it('no revalida el objeto si el id no cambia', async () => {
    vi.stubEnv('PLATFORM_TENANT_ID', TENANT);
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    const queryOne = vi
      .fn()
      .mockResolvedValueOnce(
        fila({
          background_objeto_id: OBJETO,
          background_media_type: 'video',
          background_activo: true,
        }),
      )
      .mockResolvedValueOnce(
        fila({
          background_objeto_id: OBJETO,
          background_media_type: 'video',
          background_alt: 'nuevo',
          background_activo: true,
        }),
      );
    const servicio = crear(queryOne);

    await servicio.actualizar({ background_alt: 'nuevo' }, 'admin-1');

    expect(fetchMock).not.toHaveBeenCalled();
    expect(queryOne).toHaveBeenLastCalledWith(expect.any(String), [
      OBJETO,
      'video',
      null,
      'nuevo',
      35,
      'center',
      true,
      'admin-1',
    ]);
  });

  it('limpia todo el bloque con background_objeto_id: null', async () => {
    vi.stubEnv('PLATFORM_TENANT_ID', TENANT);
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    const queryOne = vi
      .fn()
      .mockResolvedValueOnce(
        fila({
          background_objeto_id: OBJETO,
          background_media_type: 'imagen',
          background_poster_objeto_id: POSTER,
          background_alt: 'texto',
          background_overlay: 80,
          background_position: 'bottom',
          background_activo: true,
        }),
      )
      .mockResolvedValueOnce(fila({ actualizado_en: ACTUALIZADO }));
    const servicio = crear(queryOne);

    const resultado = await servicio.actualizar({ background_objeto_id: null }, 'admin-1');

    expect(fetchMock).not.toHaveBeenCalled();
    expect(queryOne).toHaveBeenLastCalledWith(expect.any(String), [
      null,
      null,
      null,
      '',
      35,
      'center',
      false,
      'admin-1',
    ]);
    expect(resultado).toMatchObject({
      background_objeto_id: null,
      background_media_type: null,
      background_poster_objeto_id: null,
      background_url: null,
      background_poster_url: null,
      background_alt: '',
      background_overlay: 35,
      background_position: 'center',
      background_activo: false,
    });
  });

  it('rechaza metadatos inválidos y activar sin objeto', async () => {
    const queryOne = vi.fn().mockResolvedValue(null);
    const servicio = crear(queryOne);

    await expect(servicio.actualizar({ background_overlay: 101 }, 'admin-1')).rejects.toMatchObject(
      { codigo: 'BACKGROUND_INVALIDO' },
    );
    await expect(servicio.actualizar({ background_activo: true }, 'admin-1')).rejects.toMatchObject(
      { codigo: 'BACKGROUND_INVALIDO' },
    );
    expect(queryOne).toHaveBeenCalledTimes(2);
  });

  it('falla si el upsert no devuelve fila', async () => {
    const queryOne = vi.fn().mockResolvedValueOnce(null).mockResolvedValueOnce(null);
    const servicio = crear(queryOne);

    await expect(servicio.actualizar({ background_alt: 'x' }, 'admin-1')).rejects.toMatchObject({
      codigo: 'CONFIGURACION_NO_ACTUALIZADA',
    });
  });
});
