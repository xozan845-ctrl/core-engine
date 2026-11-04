import type { Mock } from 'vitest';
import { ConflictError, DomainError, ForbiddenError, NotFoundError } from '@core/shared';
import { TiendasService } from './tiendas.service';

const tienda = {
  id: 't1',
  empresa_id: 'e1',
  propietario_usuario_id: 'v1',
  participacion_porcentaje: 25,
  vendedor_id: 'v1',
  nombre: 'Mi tienda',
  slug: 'mi-tienda',
  descripcion: 'venta de perifericos',
  logo_objeto_id: null,
  background_objeto_id: null,
  background_media_type: null,
  background_poster_objeto_id: null,
  background_alt: '',
  background_overlay: 35,
  background_position: 'center' as const,
  background_activo: false,
  background_url: null,
  background_poster_url: null,
  creado_en: '2026-09-01T00:00:00.000Z',
};

function crearServicio(): { servicio: TiendasService; pg: { queryOne: Mock; query: Mock } } {
  const pg = {
    queryOne: vi.fn().mockResolvedValue(null),
    query: vi.fn().mockResolvedValue([]),
  };
  return { servicio: new TiendasService(pg as never), pg };
}

describe('TiendasService (multiples tiendas por empresa)', () => {
  describe('crear (ADR-26)', () => {
    it('debe rechazar el nombre cuando tiene menos de 2 caracteres', async () => {
      const { servicio, pg } = crearServicio();
      await expect(servicio.crear('e1', 'v1', 25, 'X')).rejects.toThrow(/entre 2 y 100/);
      expect(pg.queryOne).not.toHaveBeenCalled();
    });

    it('debe rechazar el nombre cuando supera los 100 caracteres', async () => {
      const { servicio } = crearServicio();
      await expect(servicio.crear('e1', 'v1', 25, 'x'.repeat(101))).rejects.toThrow(DomainError);
    });

    it('debe derivar el slug, recortar los textos y crear la tienda', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(tienda);

      const creada = await servicio.crear('e1', 'v1', 25, '  Mi tienda  ', '  venta  ');

      expect(creada.id).toBe('t1');
      expect(pg.queryOne).toHaveBeenNthCalledWith(
        1,
        expect.stringContaining('FROM stores.tienda_slugs'),
        ['mi-tienda'],
      );
      const [sql, params] = pg.queryOne.mock.calls[1];
      expect(sql).toContain('INSERT INTO stores.tiendas');
      expect(params[1]).toBe('e1');
      expect(params[2]).toBe('v1');
      expect(params[3]).toBe('v1');
      expect(params[4]).toBe(25);
      expect(params[5]).toBe('Mi tienda');
      expect(params[6]).toBe('venta');
      expect(params[7]).toBe('mi-tienda');
    });

    it('debe crear una tienda disponible sin vendedor ni participación', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce(null).mockResolvedValueOnce({
        ...tienda,
        vendedor_id: null,
        participacion_porcentaje: null,
      });

      await servicio.crear('e1', 'emp1', null, 'Tienda disponible', undefined, undefined, null);

      const [, params] = pg.queryOne.mock.calls[1];
      expect(params[2]).toBe('emp1');
      expect(params[3]).toBeNull();
      expect(params[4]).toBeNull();
    });

    it('debe resolver una colisión de slug con sufijo -2', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce({ ok: 1 })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(tienda);

      await servicio.crear('e1', 'v1', 25, 'Mi tienda');

      expect(pg.queryOne).toHaveBeenNthCalledWith(
        2,
        expect.stringContaining('FROM stores.tienda_slugs'),
        ['mi-tienda-2'],
      );
    });

    it('debe aceptar un slug explícito normalizado', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce(null).mockResolvedValueOnce(tienda);

      await servicio.crear('e1', 'v1', 25, 'Mi tienda', undefined, '  Tienda Piloto  ');

      expect(pg.queryOne).toHaveBeenNthCalledWith(1, expect.any(String), ['tienda-piloto']);
      expect(pg.queryOne.mock.calls[1][1][7]).toBe('tienda-piloto');
    });

    it('debe rechazar un slug explícito ya usado', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce({ ok: 1 });

      await expect(
        servicio.crear('e1', 'v1', 25, 'Mi tienda', undefined, 'ocupado'),
      ).rejects.toBeInstanceOf(ConflictError);
      expect(pg.queryOne).toHaveBeenCalledTimes(1);
    });

    it('debe reintentar cuando otra petición gana la carrera del slug', async () => {
      const { servicio, pg } = crearServicio();
      let llamada = 0;
      pg.queryOne.mockImplementation(async () => {
        llamada += 1;
        if (llamada === 1) return null; // slug libre
        if (llamada === 2) throw { code: '23505' }; // INSERT pierde la carrera
        if (llamada === 3) return null; // segundo intento
        return tienda; // INSERT correcto
      });

      await expect(servicio.crear('e1', 'v1', 25, 'Mi tienda')).resolves.toEqual(tienda);
      expect(pg.queryOne).toHaveBeenCalledTimes(4);
    });

    it('debe lanzar DomainError cuando el INSERT no devuelve fila', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce(null).mockResolvedValueOnce(null);

      await expect(servicio.crear('e1', 'v1', 25, 'Mi tienda')).rejects.toThrow(/no se pudo crear/i);
    });
  });

  describe('consultas', () => {
    it('debe consultar la tienda por vendedor_id incluyendo el slug', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValue(tienda);
      expect(await servicio.deVendedor('v1')).toEqual(tienda);
      expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('slug'), ['v1']);
    });

    it('debe listar las tiendas de la empresa incluyendo el slug', async () => {
      const { servicio, pg } = crearServicio();
      pg.query.mockResolvedValue([tienda]);
      expect(await servicio.deEmpresa('e1')).toEqual([tienda]);
      expect(pg.query).toHaveBeenCalledWith(
        expect.stringContaining('WHERE empresa_id = $1'),
        ['e1'],
      );
    });

    it('debe resolver la tienda por id', async () => {
      const { servicio, pg } = crearServicio();
      const id = '11111111-2222-3333-4444-555555555555';
      pg.queryOne.mockResolvedValue({ ...tienda, id, es_actual: true });
      expect(await servicio.encontrarPorId(id)).toEqual({ ...tienda, id });
      expect(pg.queryOne).toHaveBeenCalledWith(
        expect.stringContaining('WHERE t.id = $1'),
        [id],
      );
    });
  });

  describe('resolver (UUID o slug, ADR-26)', () => {
    it('debe resolver por UUID sin redirigir', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValue({ ...tienda, es_actual: true });

      await expect(servicio.resolver(tienda.id)).resolves.toEqual({ tienda });
    });

    it('debe resolver el slug actual sin redirigir', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValue({ ...tienda, es_actual: true });

      await expect(servicio.resolver('mi-tienda')).resolves.toEqual({ tienda });
      expect(pg.queryOne).toHaveBeenCalledWith(
        expect.stringContaining('FROM stores.tienda_slugs'),
        ['mi-tienda'],
      );
    });

    it('debe redirigir al slug actual cuando el consultado es anterior', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValue({ ...tienda, slug: 'mi-tienda-nueva', es_actual: false });

      await expect(servicio.resolver('mi-tienda-vieja')).resolves.toEqual({
        tienda: { ...tienda, slug: 'mi-tienda-nueva' },
        redirigirA: '/api/v1/tiendas/mi-tienda-nueva',
      });
    });

    it('debe devolver null cuando el UUID o slug no existe', async () => {
      const { servicio } = crearServicio();
      await expect(servicio.resolver('no-existe')).resolves.toBeNull();
    });
  });

  describe('actualizarSlug (ADR-26)', () => {
    it('debe cambiar el slug cuando el actor es el propietario', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce({ ...tienda, es_actual: true })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce({ ...tienda, slug: 'mi-tienda-nueva' });

      await expect(
        servicio.actualizarSlug('v1', 't1', '  Mi Tienda Nueva  '),
      ).resolves.toEqual({ ...tienda, slug: 'mi-tienda-nueva' });
      expect(pg.queryOne).toHaveBeenNthCalledWith(
        3,
        expect.stringContaining('UPDATE stores.tiendas SET slug=$1'),
        ['mi-tienda-nueva', 't1'],
      );
    });

    it('debe ser idempotente cuando el slug no cambia', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce({ ...tienda, es_actual: true });

      await expect(servicio.actualizarSlug('v1', 't1', 'mi-tienda')).resolves.toEqual(tienda);
      expect(pg.queryOne).toHaveBeenCalledTimes(1);
    });

    it('debe rechazar a quien no es propietario', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce({ ...tienda, es_actual: true });

      await expect(servicio.actualizarSlug('u-x', 't1', 'nueva-direccion')).rejects.toBeInstanceOf(
        ForbiddenError,
      );
      expect(pg.queryOne).toHaveBeenCalledTimes(1);
    });

    it('debe rechazar un slug ya usado (incluido histórico)', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce({ ...tienda, es_actual: true })
        .mockResolvedValueOnce({ ok: 1 });

      await expect(servicio.actualizarSlug('v1', 't1', 'ocupado')).rejects.toBeInstanceOf(
        ConflictError,
      );
    });

    it('debe traducir una carrera de unicidad a ConflictError', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce({ ...tienda, es_actual: true })
        .mockResolvedValueOnce(null)
        .mockRejectedValueOnce({ code: '23505' });

      await expect(servicio.actualizarSlug('v1', 't1', 'nueva-direccion')).rejects.toBeInstanceOf(
        ConflictError,
      );
    });

    it('debe devolver NotFound si la tienda no existe', async () => {
      const { servicio } = crearServicio();
      await expect(servicio.actualizarSlug('v1', 'no-existe', 'nueva-direccion')).rejects.toBeInstanceOf(
        NotFoundError,
      );
    });
  });
});

describe('TiendasService · logo de tienda', () => {
  const logoId = '11111111-2222-3333-4444-555555555555';
  const base = { ...tienda, logo_objeto_id: null, actualizado_en: tienda.creado_en };

  afterEach(() => vi.unstubAllGlobals());

  const imagen = (publico = true) => ({
    ok: true,
    status: 200,
    json: async () => ({
      tenant_id: 'e1',
      estado: 'activo',
      publico,
      content_type: 'image/png',
    }),
  });

  it('permite al vendedor dueño y al empresario del tenant', async () => {
    const fetchMock = vi.fn(async () => imagen());
    vi.stubGlobal('fetch', fetchMock);
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ...base, es_actual: true })
        .mockResolvedValueOnce({ ...base, logo_objeto_id: logoId }),
      query: vi.fn(),
    };
    const servicio = new TiendasService(pg as never);
    const resultado = await servicio.actualizarLogo(
      { user_id: 'v1', rol: 'vendedor', tenant_id: 'e1' },
      't1',
      logoId,
    );
    expect(resultado.logo_url).toBe(`/api/v1/storage/public/objetos/${logoId}`);
    // R-RB-4: la validación interna lleva timeout explícito.
    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining(`/internal/storage/objetos/${logoId}`),
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );

    const pgEmpresa = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ...base, es_actual: true })
        .mockResolvedValueOnce({ ...base, logo_objeto_id: logoId }),
      query: vi.fn(),
    };
    const servicioEmpresa = new TiendasService(pgEmpresa as never);
    await expect(
      servicioEmpresa.actualizarLogo(
        { user_id: 'e1', rol: 'empresario', tenant_id: 'e1' },
        't1',
        logoId,
      ),
    ).resolves.toBeDefined();
  });

  it('rechaza a otro vendedor y permite admin global', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => imagen()));
    const pg = {
      queryOne: vi.fn().mockResolvedValueOnce({ ...base, es_actual: true }),
      query: vi.fn(),
    };
    const servicio = new TiendasService(pg as never);
    await expect(
      servicio.actualizarLogo({ user_id: 'v2', rol: 'vendedor', tenant_id: 'e1' }, 't1', logoId),
    ).rejects.toBeInstanceOf(ForbiddenError);

    const pgAdmin = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ...base, es_actual: true })
        .mockResolvedValueOnce({ ...base, logo_objeto_id: logoId }),
      query: vi.fn(),
    };
    const servicioAdmin = new TiendasService(pgAdmin as never);
    await expect(
      servicioAdmin.actualizarLogo({ user_id: 'admin', rol: 'admin' }, 't1', logoId),
    ).resolves.toBeDefined();
  });

  it('permite quitar el logo sin consultar storage', async () => {
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ...base, es_actual: true })
        .mockResolvedValueOnce({ ...base, logo_objeto_id: null }),
      query: vi.fn(),
    };
    const servicio = new TiendasService(pg as never);
    await expect(
      servicio.actualizarLogo({ user_id: 'v1', rol: 'vendedor', tenant_id: 'e1' }, 't1', null),
    ).resolves.toBeDefined();
  });

  it('rechaza logos inválidos, no verificables o no públicos', async () => {
    const crear = () =>
      new TiendasService({
        queryOne: vi.fn().mockResolvedValueOnce({ ...base, es_actual: true }),
        query: vi.fn(),
      } as never);

    await expect(
      crear().actualizarLogo({ user_id: 'v1', rol: 'vendedor' }, 't1', 'no-uuid'),
    ).rejects.toMatchObject({ codigo: 'LOGO_INVALIDO' });

    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 404 })));
    await expect(
      crear().actualizarLogo({ user_id: 'v1', rol: 'vendedor' }, 't1', logoId),
    ).rejects.toBeInstanceOf(NotFoundError);

    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 500 })));
    await expect(
      crear().actualizarLogo({ user_id: 'v1', rol: 'vendedor' }, 't1', logoId),
    ).rejects.toMatchObject({ codigo: 'LOGO_NO_VERIFICABLE' });

    vi.stubGlobal('fetch', vi.fn(async () => imagen(false)));
    await expect(
      crear().actualizarLogo({ user_id: 'v1', rol: 'vendedor' }, 't1', logoId),
    ).rejects.toMatchObject({ codigo: 'LOGO_INVALIDO' });
  });

  it('falla cerrado si la validación del logo se aborta por timeout', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw Object.assign(new Error('The operation was aborted'), { name: 'AbortError' });
      }),
    );
    const servicio = new TiendasService({
      queryOne: vi.fn().mockResolvedValueOnce({ ...base, es_actual: true }),
      query: vi.fn(),
    } as never);

    await expect(
      servicio.actualizarLogo({ user_id: 'v1', rol: 'vendedor' }, 't1', logoId),
    ).rejects.toMatchObject({ codigo: 'LOGO_NO_VERIFICABLE' });
  });

  it('deriva logo_url al consultar tiendas', async () => {
    const pg = {
      queryOne: vi.fn().mockResolvedValue({ ...base, logo_objeto_id: logoId }),
      query: vi.fn().mockResolvedValue([{ ...base, logo_objeto_id: logoId }]),
    };
    const servicio = new TiendasService(pg as never);
    await expect(servicio.deVendedor('v1')).resolves.toMatchObject({
      logo_url: `/api/v1/storage/public/objetos/${logoId}`,
    });
    await expect(servicio.deEmpresa('e1')).resolves.toEqual([
      expect.objectContaining({ logo_url: `/api/v1/storage/public/objetos/${logoId}` }),
    ]);
  });
});

describe('TiendasService · fondo de tienda (ADR-38)', () => {
  const fondoId = '22222222-3333-4444-8555-666666666666';
  const posterId = '33333333-4444-4555-8666-777777777777';
  const base = { ...tienda, actualizado_en: tienda.creado_en };

  afterEach(() => vi.unstubAllGlobals());

  const media = (contentType: string, extra: Record<string, unknown> = {}) => ({
    ok: true,
    status: 200,
    json: async () => ({
      tenant_id: 'e1',
      estado: 'activo',
      publico: true,
      content_type: contentType,
      ...extra,
    }),
  });

  it('valida video y poster, deriva el tipo y expone las URLs', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(media('video/mp4'))
      .mockResolvedValueOnce(media('image/webp'));
    vi.stubGlobal('fetch', fetchMock);
    const actualizada = {
      ...base,
      background_objeto_id: fondoId,
      background_media_type: 'video',
      background_poster_objeto_id: posterId,
      background_activo: true,
    };
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ...base, es_actual: true })
        .mockResolvedValueOnce(actualizada),
      query: vi.fn(),
    };
    const servicio = new TiendasService(pg as never);

    const resultado = await servicio.actualizarBackground(
      { user_id: 'v1', rol: 'vendedor', tenant_id: 'e1' },
      't1',
      {
        background_objeto_id: fondoId,
        background_poster_objeto_id: posterId,
        background_activo: true,
      },
    );

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[0][0]).toContain(`/internal/storage/objetos/${fondoId}`);
    expect(fetchMock.mock.calls[1][0]).toContain(`/internal/storage/objetos/${posterId}`);
    expect(resultado).toMatchObject({
      background_media_type: 'video',
      background_url: `/api/v1/storage/public/objetos/${fondoId}`,
      background_poster_url: `/api/v1/storage/public/objetos/${posterId}`,
      background_activo: true,
    });
    expect(pg.queryOne).toHaveBeenLastCalledWith(
      expect.stringContaining('background_objeto_id=$1'),
      [fondoId, 'video', posterId, '', 35, 'center', true, 't1'],
    );
  });

  it('no revalida los objetos cuando los ids no cambian', async () => {
    const conFondo = {
      ...base,
      background_objeto_id: fondoId,
      background_media_type: 'video' as const,
      background_poster_objeto_id: posterId,
      background_alt: 'anterior',
      background_activo: true,
    };
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ...conFondo, es_actual: true })
        .mockResolvedValueOnce({ ...conFondo, background_alt: 'nuevo' }),
      query: vi.fn(),
    };
    const servicio = new TiendasService(pg as never);

    const resultado = await servicio.actualizarBackground(
      { user_id: 'v1', rol: 'vendedor', tenant_id: 'e1' },
      't1',
      { background_alt: '  nuevo  ' },
    );

    expect(fetchMock).not.toHaveBeenCalled();
    expect(resultado).toMatchObject({
      background_objeto_id: fondoId,
      background_media_type: 'video',
      background_alt: 'nuevo',
      background_activo: true,
    });
  });

  it('limpia el bloque con background_objeto_id: null sin consultar storage', async () => {
    const conFondo = {
      ...base,
      background_objeto_id: fondoId,
      background_media_type: 'imagen' as const,
      background_poster_objeto_id: posterId,
      background_alt: 'texto',
      background_overlay: 80,
      background_position: 'bottom' as const,
      background_activo: true,
    };
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ...conFondo, es_actual: true })
        .mockResolvedValueOnce(base),
      query: vi.fn(),
    };
    const servicio = new TiendasService(pg as never);

    const resultado = await servicio.actualizarBackground(
      { user_id: 'v1', rol: 'vendedor', tenant_id: 'e1' },
      't1',
      { background_objeto_id: null },
    );

    expect(fetchMock).not.toHaveBeenCalled();
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

  it('aplica la misma autorización que el logo', async () => {
    const crear = () =>
      new TiendasService({
        queryOne: vi.fn().mockResolvedValueOnce({ ...base, es_actual: true }),
        query: vi.fn(),
      } as never);

    await expect(
      crear().actualizarBackground(
        { user_id: 'v2', rol: 'vendedor', tenant_id: 'e1' },
        't1',
        { background_alt: 'x' },
      ),
    ).rejects.toBeInstanceOf(ForbiddenError);

    vi.stubGlobal('fetch', vi.fn(async () => media('image/png')));
    const pgAdmin = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ...base, es_actual: true })
        .mockResolvedValueOnce({ ...base, background_objeto_id: fondoId }),
      query: vi.fn(),
    };
    await expect(
      new TiendasService(pgAdmin as never).actualizarBackground(
        { user_id: 'admin', rol: 'admin' },
        't1',
        { background_objeto_id: fondoId },
      ),
    ).resolves.toBeDefined();
  });

  it('rechaza activar sin fondo y un poster de video', async () => {
    const crear = (actual = base) =>
      new TiendasService({
        queryOne: vi.fn().mockResolvedValueOnce({ ...actual, es_actual: true }),
        query: vi.fn(),
      } as never);

    await expect(
      crear().actualizarBackground(
        { user_id: 'v1', rol: 'vendedor', tenant_id: 'e1' },
        't1',
        { background_activo: true },
      ),
    ).rejects.toMatchObject({ codigo: 'BACKGROUND_INVALIDO' });

    const conFondo = {
      ...base,
      background_objeto_id: fondoId,
      background_media_type: 'imagen' as const,
    };
    vi.stubGlobal('fetch', vi.fn(async () => media('video/mp4')));
    await expect(
      crear(conFondo).actualizarBackground(
        { user_id: 'v1', rol: 'vendedor', tenant_id: 'e1' },
        't1',
        { background_poster_objeto_id: posterId },
      ),
    ).rejects.toMatchObject({ codigo: 'BACKGROUND_INVALIDO' });
  });

  it('expone las URLs del fondo al consultar tiendas', async () => {
    const conFondo = {
      ...base,
      background_objeto_id: fondoId,
      background_media_type: 'imagen' as const,
      background_poster_objeto_id: posterId,
    };
    const pg = {
      queryOne: vi.fn().mockResolvedValue({ ...conFondo, es_actual: true }),
      query: vi.fn().mockResolvedValue([conFondo]),
    };
    const servicio = new TiendasService(pg as never);

    await expect(servicio.deVendedor('v1')).resolves.toMatchObject({
      background_url: `/api/v1/storage/public/objetos/${fondoId}`,
      background_poster_url: `/api/v1/storage/public/objetos/${posterId}`,
    });
    await expect(servicio.deEmpresa('e1')).resolves.toEqual([
      expect.objectContaining({
        background_url: `/api/v1/storage/public/objetos/${fondoId}`,
      }),
    ]);
  });
});
