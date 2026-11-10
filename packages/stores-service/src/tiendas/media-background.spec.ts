import { DomainError, NotFoundError } from '@core/shared';
import {
  BACKGROUND_MOVIL_POR_DEFECTO,
  BACKGROUND_POR_DEFECTO,
  ValidadorMediaBackground,
  backgroundDe,
  backgroundMovilDe,
  conBackgroundMovilUrls,
  conBackgroundUrls,
  normalizarBackground,
  normalizarBackgroundMovil,
  validarBackground,
  validarBackgroundMovil,
} from './media-background';

const OBJETO = '11111111-2222-4333-8444-555555555555';
const POSTER = '99999999-8888-4777-8666-555555555555';
const TENANT = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';

describe('media background (ADR-38)', () => {
  afterEach(() => vi.unstubAllGlobals());

  describe('normalizarBackground', () => {
    it('devuelve los valores por defecto sin datos ni fila previa', () => {
      expect(normalizarBackground({})).toEqual(BACKGROUND_POR_DEFECTO);
    });

    it('mezcla solo los campos enviados y conserva el tipo si el objeto no cambia', () => {
      const actual = {
        ...BACKGROUND_POR_DEFECTO,
        background_objeto_id: OBJETO,
        background_media_type: 'video' as const,
        background_alt: 'anterior',
        background_activo: true,
      };

      expect(normalizarBackground({ background_alt: '  nuevo  ' }, actual)).toMatchObject({
        background_objeto_id: OBJETO,
        background_media_type: 'video',
        background_alt: 'nuevo',
        background_activo: true,
      });
    });

    it('invalida el tipo derivado cuando el objeto cambia', () => {
      const actual = {
        ...BACKGROUND_POR_DEFECTO,
        background_objeto_id: OBJETO,
        background_media_type: 'video' as const,
      };

      expect(
        normalizarBackground({ background_objeto_id: POSTER }, actual).background_media_type,
      ).toBeNull();
    });

    it('conserva los metadatos aunque el fondo aun no exista', () => {
      expect(
        normalizarBackground({ background_alt: '  Próximamente  ', background_overlay: 60 }),
      ).toMatchObject({
        background_objeto_id: null,
        background_media_type: null,
        background_alt: 'Próximamente',
        background_overlay: 60,
      });
    });

    it('limpia todo el bloque cuando el objeto es null', () => {
      const actual = {
        ...BACKGROUND_POR_DEFECTO,
        background_objeto_id: OBJETO,
        background_media_type: 'imagen' as const,
        background_poster_objeto_id: POSTER,
        background_alt: 'texto',
        background_overlay: 80,
        background_position: 'top' as const,
        background_activo: true,
      };

      expect(normalizarBackground({ background_objeto_id: null }, actual)).toEqual(
        BACKGROUND_POR_DEFECTO,
      );
    });
  });

  describe('backgroundDe', () => {
    it('devuelve los valores por defecto para una fila ausente', () => {
      expect(backgroundDe(null)).toEqual(BACKGROUND_POR_DEFECTO);
      expect(backgroundDe(undefined)).toEqual(BACKGROUND_POR_DEFECTO);
    });

    it('completa solo los campos presentes en una fila parcial', () => {
      expect(
        backgroundDe({
          background_objeto_id: OBJETO,
          background_media_type: 'video',
          background_activo: true,
        }),
      ).toEqual({
        ...BACKGROUND_POR_DEFECTO,
        background_objeto_id: OBJETO,
        background_media_type: 'video',
        background_activo: true,
      });
    });
  });

  describe('validarBackground', () => {
    const valido = () => ({ ...BACKGROUND_POR_DEFECTO });

    it('acepta el bloque vacio y un fondo válido', () => {
      expect(() => validarBackground({}, valido())).not.toThrow();
      expect(() =>
        validarBackground(
          {},
          { ...valido(), background_objeto_id: OBJETO, background_media_type: 'imagen' },
        ),
      ).not.toThrow();
    });

    it('rechaza activar o poner poster sin objeto de fondo', () => {
      expect(() => validarBackground({ background_activo: true }, valido())).toThrow(DomainError);
      expect(() =>
        validarBackground({ background_poster_objeto_id: POSTER }, valido()),
      ).toThrow(/poster requiere/i);
    });

    it('rechaza ids que no son uuid', () => {
      expect(() =>
        validarBackground({}, { ...valido(), background_objeto_id: 'no-uuid' }),
      ).toThrow(/objeto de storage/i);
      expect(() =>
        validarBackground(
          {},
          { ...valido(), background_objeto_id: OBJETO, background_poster_objeto_id: 'x' },
        ),
      ).toThrow(/poster/i);
    });

    it('rechaza metadatos fuera de rango', () => {
      expect(() =>
        validarBackground({}, { ...valido(), background_alt: 'x'.repeat(161) }),
      ).toThrow(/160/);
      for (const overlay of [-1, 101, 1.5]) {
        expect(() => validarBackground({}, { ...valido(), background_overlay: overlay })).toThrow(
          /overlay/,
        );
      }
      expect(() =>
        validarBackground(
          {},
          { ...valido(), background_position: 'diagonal' as never },
        ),
      ).toThrow(/posición/);
      expect(() =>
        validarBackground({}, { ...valido(), background_activo: 'si' as never }),
      ).toThrow(/booleano/);
    });
  });

  describe('conBackgroundUrls', () => {
    it('deriva las rutas publicas de fondo y poster', () => {
      expect(
        conBackgroundUrls({
          ...BACKGROUND_POR_DEFECTO,
          background_objeto_id: OBJETO,
          background_media_type: 'video',
          background_poster_objeto_id: POSTER,
        }),
      ).toMatchObject({
        background_url: `/api/v2/storage/public/objetos/${OBJETO}`,
        background_poster_url: `/api/v2/storage/public/objetos/${POSTER}`,
      });
    });

    it('devuelve null cuando no hay ids', () => {
      expect(conBackgroundUrls({ ...BACKGROUND_POR_DEFECTO })).toMatchObject({
        background_url: null,
        background_poster_url: null,
      });
    });
  });

  describe('ValidadorMediaBackground', () => {
    const validador = new ValidadorMediaBackground('http://storage', 'clave');
    const respuesta = (objeto: Record<string, unknown>): Response =>
      ({
        ok: true,
        status: 200,
        json: async () => ({
          id: OBJETO,
          tenant_id: TENANT,
          estado: 'activo',
          publico: true,
          content_type: 'image/jpeg',
          ...objeto,
        }),
      }) as unknown as Response;

    it('deriva imagen o video y envia la clave interna con timeout', async () => {
      const fetchMock = vi.fn(async () => respuesta({ content_type: 'video/mp4' }));
      vi.stubGlobal('fetch', fetchMock);

      await expect(validador.validar(TENANT, OBJETO)).resolves.toBe('video');
      expect(fetchMock).toHaveBeenCalledWith(
        `http://storage/internal/storage/objetos/${OBJETO}?tenant_id=${TENANT}`,
        expect.objectContaining({
          headers: { 'x-internal-key': 'clave' },
          signal: expect.any(AbortSignal),
        }),
      );

      vi.stubGlobal('fetch', vi.fn(async () => respuesta({ content_type: 'image/webp' })));
      await expect(validador.validar(TENANT, OBJETO)).resolves.toBe('imagen');
    });

    it('acepta solo imagenes como poster', async () => {
      vi.stubGlobal('fetch', vi.fn(async () => respuesta({ content_type: 'image/png' })));
      await expect(validador.validar(TENANT, POSTER, 'poster')).resolves.toBe('imagen');

      vi.stubGlobal('fetch', vi.fn(async () => respuesta({ content_type: 'video/mp4' })));
      await expect(validador.validar(TENANT, POSTER, 'poster')).rejects.toMatchObject({
        codigo: 'BACKGROUND_INVALIDO',
      });
    });

    it('acepta solo imagenes para la imagen SEO de tienda', async () => {
      vi.stubGlobal('fetch', vi.fn(async () => respuesta({ content_type: 'image/webp' })));
      await expect(validador.validar(TENANT, POSTER, 'imagen')).resolves.toBe('imagen');

      vi.stubGlobal('fetch', vi.fn(async () => respuesta({ content_type: 'video/mp4' })));
      await expect(validador.validar(TENANT, POSTER, 'imagen')).rejects.toMatchObject({
        codigo: 'BACKGROUND_INVALIDO',
      });
    });

    it('rechaza objetos de otro tenant, inactivos, privados o con MIME no permitido', async () => {
      for (const objeto of [
        { tenant_id: 'otro' },
        { estado: 'pendiente' },
        { publico: false },
        { content_type: 'image/svg+xml' },
        { content_type: 'application/pdf' },
      ]) {
        vi.stubGlobal('fetch', vi.fn(async () => respuesta(objeto)));
        await expect(validador.validar(TENANT, OBJETO)).rejects.toMatchObject({
          codigo: 'BACKGROUND_INVALIDO',
        });
      }
    });

    it('traduce 404 a NotFoundError y fallos de red a BACKGROUND_NO_VERIFICABLE', async () => {
      vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 404 })));
      await expect(validador.validar(TENANT, OBJETO)).rejects.toBeInstanceOf(NotFoundError);
      await expect(validador.validar(TENANT, POSTER, 'poster')).rejects.toThrow(/Poster/);

      vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 500 })));
      await expect(validador.validar(TENANT, OBJETO)).rejects.toMatchObject({
        codigo: 'BACKGROUND_NO_VERIFICABLE',
      });

      vi.stubGlobal(
        'fetch',
        vi.fn(async () => {
          throw new Error('sin red');
        }),
      );
      await expect(validador.validar(TENANT, OBJETO)).rejects.toMatchObject({
        codigo: 'BACKGROUND_NO_VERIFICABLE',
      });
    });
  });

  describe('fondo móvil (ADR-41)', () => {
    it('devuelve los valores por defecto sin fila', () => {
      expect(backgroundMovilDe(null)).toEqual(BACKGROUND_MOVIL_POR_DEFECTO);
      expect(normalizarBackgroundMovil({})).toEqual(BACKGROUND_MOVIL_POR_DEFECTO);
    });

    it('mezcla solo los campos enviados y conserva el tipo si el objeto no cambia', () => {
      const actual = {
        ...BACKGROUND_MOVIL_POR_DEFECTO,
        background_movil_objeto_id: OBJETO,
        background_movil_media_type: 'video' as const,
        background_movil_alt: 'anterior',
        background_movil_activo: true,
      };

      expect(
        normalizarBackgroundMovil({ background_movil_alt: '  nuevo  ' }, actual),
      ).toMatchObject({
        background_movil_objeto_id: OBJETO,
        background_movil_media_type: 'video',
        background_movil_alt: 'nuevo',
        background_movil_activo: true,
      });
    });

    it('limpia el bloque móvil con null', () => {
      const actual = {
        ...BACKGROUND_MOVIL_POR_DEFECTO,
        background_movil_objeto_id: OBJETO,
        background_movil_media_type: 'imagen' as const,
        background_movil_activo: true,
      };
      expect(
        normalizarBackgroundMovil({ background_movil_objeto_id: null }, actual),
      ).toEqual(BACKGROUND_MOVIL_POR_DEFECTO);
    });

    it('rechaza activar o poner poster sin objeto móvil', () => {
      expect(() =>
        validarBackgroundMovil(
          { background_movil_activo: true },
          { ...BACKGROUND_MOVIL_POR_DEFECTO },
        ),
      ).toThrow(/fondo móvil/i);
      expect(() =>
        validarBackgroundMovil(
          { background_movil_poster_objeto_id: POSTER },
          { ...BACKGROUND_MOVIL_POR_DEFECTO },
        ),
      ).toThrow(/poster móvil/i);
    });

    it('valida UUIDs y el texto alternativo', () => {
      expect(() =>
        validarBackgroundMovil(
          { background_movil_objeto_id: OBJETO },
          { ...BACKGROUND_MOVIL_POR_DEFECTO, background_movil_objeto_id: 'x' },
        ),
      ).toThrow(/objeto de storage/i);
      expect(() =>
        validarBackgroundMovil(
          {},
          {
            ...BACKGROUND_MOVIL_POR_DEFECTO,
            background_movil_objeto_id: OBJETO,
            background_movil_alt: 'x'.repeat(161),
          },
        ),
      ).toThrow(/160/);
    });

    it('deriva las URLs públicas móviles', () => {
      expect(
        conBackgroundMovilUrls({
          ...BACKGROUND_MOVIL_POR_DEFECTO,
          background_movil_objeto_id: OBJETO,
          background_movil_poster_objeto_id: POSTER,
        }),
      ).toMatchObject({
        background_movil_url: `/api/v2/storage/public/objetos/${OBJETO}`,
        background_movil_poster_url: `/api/v2/storage/public/objetos/${POSTER}`,
      });
      expect(conBackgroundMovilUrls(BACKGROUND_MOVIL_POR_DEFECTO)).toMatchObject({
        background_movil_url: null,
        background_movil_poster_url: null,
      });
    });
  });
});
