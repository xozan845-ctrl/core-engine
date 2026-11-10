import type { Mock } from 'vitest';
import { ConflictError, ForbiddenError, NotFoundError } from '@core/shared';
import { EmpresasService } from './empresas.service';

const empresa = {
  id: 'e-1',
  nombre: 'Acme',
  slug: 'acme',
  estado: 'activa',
  creado_por: 'u-1',
  creado_en: '2026-10-23T00:00:00.000Z',
  background_objeto_id: null,
  background_media_type: null,
  background_poster_objeto_id: null,
  background_alt: '',
  background_overlay: 35,
  background_position: 'center' as const,
  background_activo: false,
  background_url: null,
  background_poster_url: null,
  plantilla: 'clasica',
  tema: {
    color_primario: '#1F2937',
    color_secundario: '#F59E0B',
    color_acento: '#10B981',
    color_fondo: '#FFFFFF',
    color_texto: '#111827',
    fuente: 'system',
    radio_bordes: 12,
    modo: 'claro',
  },
  origen_personalizacion: { plantilla: 'defecto', tema: 'defecto' },
};

function clienteInsertable(): { query: Mock } {
  return {
    query: vi.fn()
      .mockResolvedValueOnce({ rows: [], rowCount: 0 }) // slug libre
      .mockResolvedValueOnce({ rows: [empresa], rowCount: 1 }) // INSERT empresa
      .mockResolvedValueOnce({ rows: [], rowCount: 1 }) // INSERT miembro
      .mockResolvedValueOnce({ rows: [], rowCount: 1 }) // UPDATE usuario
      .mockResolvedValueOnce({ rows: [], rowCount: 1 }), // capacidades por vertical
  };
}

describe('EmpresasService', () => {
  describe('crear (ADR-26)', () => {
    it('deriva el slug del nombre, crea la empresa y activa el tenant', async () => {
      const client = clienteInsertable();
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.crear('u-1', '  Acme  ')).resolves.toEqual(empresa);

      expect(client.query).toHaveBeenNthCalledWith(
        1,
        expect.stringContaining('FROM identity.empresa_slugs'),
        ['acme'],
      );
      expect(client.query).toHaveBeenNthCalledWith(
        2,
        expect.stringContaining('INSERT INTO identity.empresas'),
        expect.arrayContaining(['Acme', 'acme', 'retail', 'pack_retail', 1, 'u-1']),
      );
      expect(client.query).toHaveBeenCalledTimes(7);
    });

    it('debe aplicar el pack seleccionado y sus capacidades al crear el tenant', async () => {
      const client = {
        query: vi.fn()
          .mockResolvedValueOnce({ rows: [{ codigo: 'pack_belleza', version: 1, vertical_codigo: 'belleza' }], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 0 })
          .mockResolvedValueOnce({ rows: [empresa], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 }),
      };
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await servicio.crear('u-1', 'Salon Acme', undefined, undefined, undefined, 'pack_belleza');

      expect(client.query).toHaveBeenNthCalledWith(1, expect.stringContaining('FROM identity.packs'), ['pack_belleza']);
      expect(client.query).toHaveBeenNthCalledWith(3, expect.stringContaining('pack_codigo, pack_version'), expect.arrayContaining(['Salon Acme', 'salon-acme', 'belleza', 'pack_belleza', 1]));
      expect(client.query).toHaveBeenNthCalledWith(7, expect.stringContaining('FROM identity.pack_capacidades'), [empresa.id, 'pack_belleza', 1]);
    });

    it('debe rechazar un pack que no corresponde a la vertical solicitada', async () => {
      const client = { query: vi.fn().mockResolvedValueOnce({ rows: [{ codigo: 'pack_belleza', version: 1, vertical_codigo: 'belleza' }], rowCount: 1 }) };
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.crear('u-1', 'Acme', undefined, undefined, 'retail', 'pack_belleza')).rejects.toThrow('vertical solicitada');
    });

    it('resuelve una colisión de slug automático con sufijo -2', async () => {
      const client = {
        query: vi.fn()
          .mockResolvedValueOnce({ rows: [{ ok: 1 }], rowCount: 1 }) // acme ocupado
          .mockResolvedValueOnce({ rows: [], rowCount: 0 }) // acme-2 libre
          .mockResolvedValueOnce({ rows: [empresa], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 }) // UPDATE usuario
          .mockResolvedValueOnce({ rows: [], rowCount: 1 }), // capacidades por vertical
      };
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await servicio.crear('u-1', 'Acme');

      expect(client.query).toHaveBeenNthCalledWith(2, expect.any(String), ['acme-2']);
      expect(client.query).toHaveBeenNthCalledWith(
        3,
        expect.stringContaining('INSERT INTO identity.empresas'),
        expect.arrayContaining(['Acme', 'acme-2', 'retail', 'pack_retail', 1, 'u-1']),
      );
    });

    it('acepta un slug explícito normalizado', async () => {
      const client = {
        query: vi.fn()
          .mockResolvedValueOnce({ rows: [], rowCount: 0 })
          .mockResolvedValueOnce({ rows: [empresa], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 }),
      };
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await servicio.crear('u-1', 'Acme', '  Mi Empresa  ');

      expect(client.query).toHaveBeenNthCalledWith(1, expect.any(String), ['mi-empresa']);
      expect(client.query).toHaveBeenNthCalledWith(
        2,
        expect.stringContaining('INSERT INTO identity.empresas'),
        expect.arrayContaining(['Acme', 'mi-empresa', 'retail', 'pack_retail', 1, 'u-1']),
      );
    });

    it('rechaza con 409 un slug explícito ya usado (incluido histórico)', async () => {
      const client = { query: vi.fn().mockResolvedValueOnce({ rows: [{ ok: 1 }], rowCount: 1 }) };
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.crear('u-1', 'Acme', 'acme')).rejects.toBeInstanceOf(
        ConflictError,
      );
      expect(client.query).toHaveBeenCalledTimes(1);
    });

    it('reintenta la transacción cuando otra petición gana la carrera del slug', async () => {
      const client = clienteInsertable();
      const pg = {
        transaccion: vi.fn()
          .mockRejectedValueOnce({ code: '23505' })
          .mockImplementationOnce(async (fn: (c: typeof client) => unknown) => fn(client)),
      };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.crear('u-1', 'Acme')).resolves.toEqual(empresa);
      expect(pg.transaccion).toHaveBeenCalledTimes(2);
    });

    it('traduce una violación UNIQUE de un slug explícito a ConflictError', async () => {
      const pg = { transaccion: vi.fn().mockRejectedValue({ code: '23505' }) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.crear('u-1', 'Acme', 'acme')).rejects.toBeInstanceOf(
        ConflictError,
      );
    });

    it('usa un sufijo aleatorio cuando los 50 intentos automáticos están ocupados', async () => {
      const client = {
        query: vi.fn().mockImplementation(async (sql: string) => {
          if (sql.includes('FROM identity.empresa_slugs')) {
            return { rows: [{ ok: 1 }], rowCount: 1 };
          }
          if (sql.includes('INSERT INTO identity.empresas')) {
            return { rows: [empresa], rowCount: 1 };
          }
          return { rows: [], rowCount: 1 };
        }),
      };
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.crear('u-1', 'Acme')).resolves.toEqual(empresa);

      const insert = client.query.mock.calls.find(([sql]) => String(sql).includes('INSERT INTO identity.empresas'));
      expect(insert?.[1]?.[1]).toMatch(/^acme-[a-z0-9]+$/);
      expect(String(insert?.[1]?.[1]).length).toBeLessThanOrEqual(80);
    });
  });

  it('crea un vendedor cuando el empresario pertenece a la empresa', async () => {
    const usuario = { id: 'v-1', rol: 'vendedor' };
    const pg = { queryOne: vi.fn().mockResolvedValue({ ok: 1 }) };
    const usuarios = { crearVendedorEnEmpresa: vi.fn().mockResolvedValue(usuario) };
    const servicio = new EmpresasService(pg as never, usuarios as never);
    const datos = {
      nombre: 'Venta', correo: 'venta@acme.test', contrasena: 'secreto123',
      participacion_porcentaje: 25,
    };

    await expect(servicio.crearVendedor('u-1', 'e-1', datos))
      .resolves.toEqual({ ...usuario, participacion_porcentaje: 25 });
    expect(usuarios.crearVendedorEnEmpresa).toHaveBeenCalledWith({ ...datos, empresa_id: 'e-1' });
  });

  it('impide crear vendedores a quien no es empresario activo', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue(null) };
    const usuarios = { crearVendedorEnEmpresa: vi.fn() };
    const servicio = new EmpresasService(pg as never, usuarios as never);
    await expect(servicio.crearVendedor('u-x', 'e-1', {
      nombre: 'Venta', correo: 'venta@acme.test', contrasena: 'secreto123',
      participacion_porcentaje: 25,
    })).rejects.toBeInstanceOf(ForbiddenError);
    expect(usuarios.crearVendedorEnEmpresa).not.toHaveBeenCalled();
  });

  describe('resolverPara (UUID o slug, ADR-26)', () => {
    it('resuelve un UUID como administrador global sin consultar membresía', async () => {
      const pg = { queryOne: vi.fn().mockResolvedValue({ ...empresa, es_actual: true }) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.resolverPara('sa-1', 'e-1', true)).resolves.toMatchObject({ empresa });
      expect(pg.queryOne).toHaveBeenCalledTimes(1);
    });

    it('resuelve el slug actual para un miembro activo', async () => {
      const pg = {
        queryOne: vi.fn()
          .mockResolvedValueOnce({ ...empresa, es_actual: true })
          .mockResolvedValueOnce({ ok: 1 }),
      };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.resolverPara('u-1', 'acme', false)).resolves.toMatchObject({ empresa });
      expect(pg.queryOne).toHaveBeenNthCalledWith(1, expect.stringContaining('identity.empresa_slugs'), ['acme']);
    });

    it('devuelve la ruta actual cuando el slug consultado es anterior', async () => {
      const pg = {
        queryOne: vi.fn()
          .mockResolvedValueOnce({ ...empresa, slug: 'acme-nueva', es_actual: false })
          .mockResolvedValueOnce({ ok: 1 }),
      };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.resolverPara('u-1', 'acme-vieja', false)).resolves.toMatchObject({
        empresa: { ...empresa, slug: 'acme-nueva' },
        redirigirA: '/api/v2/empresas/acme-nueva',
      });
    });

    it('oculta una empresa a quien no tiene membresía', async () => {
      const pg = {
        queryOne: vi.fn()
          .mockResolvedValueOnce({ ...empresa, es_actual: true })
          .mockResolvedValueOnce(null),
      };
      const servicio = new EmpresasService(pg as never, {} as never);
      await expect(servicio.resolverPara('u-x', 'acme', false)).rejects.toBeInstanceOf(NotFoundError);
    });

    it('devuelve 404 si el UUID o slug no existe', async () => {
      const pg = { queryOne: vi.fn().mockResolvedValue(null) };
      const servicio = new EmpresasService(pg as never, {} as never);
      await expect(servicio.resolverPara('sa-1', 'no-existe', true)).rejects.toBeInstanceOf(
        NotFoundError,
      );
    });

    it('obtiene una empresa por UUID para un miembro (obtenerPara)', async () => {
      const id = '11111111-2222-4333-8444-555555555555';
      const pg = {
        queryOne: vi.fn()
          .mockResolvedValueOnce({ ...empresa, id, es_actual: true })
          .mockResolvedValueOnce({ ok: 1 }),
      };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.obtenerPara('u-1', id, false)).resolves.toEqual({ ...empresa, id });
      expect(pg.queryOne).toHaveBeenNthCalledWith(
        1,
        expect.stringContaining('WHERE id=$1'),
        [id],
      );
    });
  });

  describe('actualizarSlug (ADR-26)', () => {
    function servicioCon(secuencia: unknown[]): { servicio: EmpresasService; pg: { queryOne: Mock } } {
      const pg = { queryOne: vi.fn() };
      for (const valor of secuencia) (pg.queryOne as Mock).mockResolvedValueOnce(valor);
      return { servicio: new EmpresasService(pg as never, {} as never), pg };
    }

    it('cambia el slug del empresario miembro y conserva el anterior en el historial', async () => {
      const { servicio, pg } = servicioCon([
        { ...empresa, es_actual: true }, // resolver UUID
        { ok: 1 }, // membresía
        null, // slug libre
        { ...empresa, slug: 'acme-nueva' }, // UPDATE
      ]);

      await expect(servicio.actualizarSlug('u-1', 'e-1', '  Acme Nueva  ')).resolves.toEqual({
        ...empresa,
        slug: 'acme-nueva',
      });
      expect(pg.queryOne).toHaveBeenNthCalledWith(
        4,
        expect.stringContaining('UPDATE identity.empresas SET slug=$1'),
        ['acme-nueva', 'e-1'],
      );
    });

    it('es idempotente cuando el slug no cambia', async () => {
      const { servicio, pg } = servicioCon([
        { ...empresa, es_actual: true },
        { ok: 1 },
      ]);

      await expect(servicio.actualizarSlug('u-1', 'e-1', 'acme')).resolves.toEqual(empresa);
      expect(pg.queryOne).toHaveBeenCalledTimes(2);
    });

    it('rechaza un slug que ya existe en el historial', async () => {
      const { servicio } = servicioCon([
        { ...empresa, es_actual: true },
        { ok: 1 },
        { ok: 1 },
      ]);

      await expect(servicio.actualizarSlug('u-1', 'e-1', 'ocupado')).rejects.toBeInstanceOf(
        ConflictError,
      );
    });

    it('traduce una carrera de unicidad a ConflictError', async () => {
      const { servicio, pg } = servicioCon([
        { ...empresa, es_actual: true },
        { ok: 1 },
        null,
      ]);
      pg.queryOne.mockRejectedValueOnce({ code: '23505' });

      await expect(servicio.actualizarSlug('u-1', 'e-1', 'nuevo-slug')).rejects.toBeInstanceOf(
        ConflictError,
      );
    });

    it('no permite cambiar el slug a un no miembro', async () => {
      const { servicio } = servicioCon([{ ...empresa, es_actual: true }, null]);
      await expect(servicio.actualizarSlug('u-x', 'e-1', 'nuevo-slug')).rejects.toBeInstanceOf(
        NotFoundError,
      );
    });

    it('propaga errores no relacionados con la unicidad', async () => {
      const { servicio, pg } = servicioCon([
        { ...empresa, es_actual: true },
        { ok: 1 },
        null,
      ]);
      pg.queryOne.mockRejectedValueOnce(new Error('base de datos caída'));

      await expect(servicio.actualizarSlug('u-1', 'e-1', 'otro-slug')).rejects.toThrow(
        'base de datos caída',
      );
    });
  });

  it('expone la consulta interna por id con su slug', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue(empresa) };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(servicio.obtener('e-1')).resolves.toEqual(empresa);
  });
});

describe('EmpresasService · perfil y logo', () => {
  afterEach(() => vi.unstubAllGlobals());

  const logoId = '11111111-2222-3333-4444-555555555555';

  it('actualiza metadatos parciales del perfil', async () => {
    const actualizada = { ...empresa, ruc: 'RUC-001', telefono: '8888', moneda: 'NIO' };
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce(empresa) // resolver por id
        .mockResolvedValueOnce({ ok: 1 }) // membresía empresario
        .mockResolvedValueOnce(null) // RUC libre
        .mockResolvedValueOnce(actualizada), // UPDATE
    };
    const servicio = new EmpresasService(pg as never, {} as never);

    const resultado = await servicio.actualizarPerfil('u-1', 'e-1', {
      ruc: 'RUC-001',
      telefono: '8888',
    });

    expect(resultado).toMatchObject({ ruc: 'RUC-001', telefono: '8888' });
    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining('UPDATE identity.empresas'),
      expect.arrayContaining(['e-1', 'RUC-001', '8888']),
    );
  });

  it('permite que un admin actualice cualquier empresa', async () => {
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce(empresa)
        .mockResolvedValueOnce({ ...empresa, sector: 'tecnologia' }),
    };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(
      servicio.actualizarPerfil('admin-1', 'e-1', { sector: 'tecnologia' }, true),
    ).resolves.toMatchObject({ sector: 'tecnologia' });
  });

  it('valida el logo contra storage y expone logo_url', async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({
        id: logoId,
        tenant_id: 'e-1',
        estado: 'activo',
        publico: true,
        content_type: 'image/png',
      }),
    }));
    vi.stubGlobal('fetch', fetchMock);
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce(empresa)
        .mockResolvedValueOnce({ ok: 1 })
        .mockResolvedValueOnce({ ...empresa, logo_objeto_id: logoId }),
    };
    const servicio = new EmpresasService(pg as never, {} as never);

    const resultado = await servicio.actualizarPerfil('u-1', 'e-1', {
      logo_objeto_id: logoId,
    });

    expect(resultado.logo_url).toBe(`/api/v2/storage/public/objetos/${logoId}`);
    // R-RB-4: la validación interna lleva timeout explícito.
    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining(`/internal/storage/objetos/${logoId}`),
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it('falla cerrado si la validación del logo se aborta por timeout', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw Object.assign(new Error('The operation was aborted'), { name: 'AbortError' });
      }),
    );
    const pg = {
      queryOne: vi.fn().mockResolvedValueOnce(empresa).mockResolvedValueOnce({ ok: 1 }),
    };
    const servicio = new EmpresasService(pg as never, {} as never);

    await expect(
      servicio.actualizarPerfil('u-1', 'e-1', { logo_objeto_id: logoId }),
    ).rejects.toMatchObject({ codigo: 'LOGO_NO_VERIFICABLE' });
  });

  it('rechaza RUC duplicado', async () => {
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce(empresa)
        .mockResolvedValueOnce({ ok: 1 })
        .mockResolvedValueOnce({ id: 'otra' }),
    };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(
      servicio.actualizarPerfil('u-1', 'e-1', { ruc: 'RUC-001' }),
    ).rejects.toBeInstanceOf(ConflictError);
  });

  it('rechaza moneda, correo y logo inválidos', async () => {
    const crearServicio = () =>
      new EmpresasService(
        {
          queryOne: vi.fn()
            .mockResolvedValueOnce(empresa)
            .mockResolvedValueOnce({ ok: 1 }),
        } as never,
        {} as never,
      );

    await expect(
      crearServicio().actualizarPerfil('u-1', 'e-1', { moneda: 'NI' }),
    ).rejects.toMatchObject({ codigo: 'PERFIL_INVALIDO' });
    await expect(
      crearServicio().actualizarPerfil('u-1', 'e-1', { correo_contacto: 'no-es-correo' }),
    ).rejects.toMatchObject({ codigo: 'PERFIL_INVALIDO' });
    await expect(
      crearServicio().actualizarPerfil('u-1', 'e-1', { logo_objeto_id: 'no-uuid' }),
    ).rejects.toMatchObject({ codigo: 'PERFIL_INVALIDO' });
  });

  it('valida fondo de video y poster, deriva el tipo y expone las URLs', async () => {
    const fondoId = '22222222-3333-4444-8555-666666666666';
    const posterId = '33333333-4444-4555-8666-777777777777';
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => ({
          tenant_id: 'e-1',
          estado: 'activo',
          publico: true,
          content_type: 'video/mp4',
        }),
      })
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => ({
          tenant_id: 'e-1',
          estado: 'activo',
          publico: true,
          content_type: 'image/webp',
        }),
      });
    vi.stubGlobal('fetch', fetchMock);
    const actualizada = {
      ...empresa,
      background_objeto_id: fondoId,
      background_media_type: 'video',
      background_poster_objeto_id: posterId,
      background_activo: true,
    };
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce(empresa)
        .mockResolvedValueOnce({ ok: 1 })
        .mockResolvedValueOnce(actualizada),
    };
    const servicio = new EmpresasService(pg as never, {} as never);

    const resultado = await servicio.actualizarPerfil('u-1', 'e-1', {
      background_objeto_id: fondoId,
      background_poster_objeto_id: posterId,
      background_activo: true,
    });

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[0][0]).toContain(`/internal/storage/objetos/${fondoId}`);
    expect(fetchMock.mock.calls[1][0]).toContain(`/internal/storage/objetos/${posterId}`);
    expect(resultado).toMatchObject({
      background_media_type: 'video',
      background_url: `/api/v2/storage/public/objetos/${fondoId}`,
      background_poster_url: `/api/v2/storage/public/objetos/${posterId}`,
      background_activo: true,
    });
    expect(pg.queryOne).toHaveBeenLastCalledWith(
      expect.stringContaining('background_objeto_id=$19'),
      expect.arrayContaining(['e-1', fondoId, 'video', posterId, '', 35, 'center', true]),
    );
  });

  it('no revalida fondo ni poster cuando los ids no cambian', async () => {
    const fondoId = '22222222-3333-4444-8555-666666666666';
    const posterId = '33333333-4444-4555-8666-777777777777';
    const conFondo = {
      ...empresa,
      background_objeto_id: fondoId,
      background_media_type: 'video',
      background_poster_objeto_id: posterId,
      background_alt: 'anterior',
      background_activo: true,
    };
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce(conFondo)
        .mockResolvedValueOnce({ ok: 1 })
        .mockResolvedValueOnce({ ...conFondo, background_alt: 'nuevo' }),
    };
    const servicio = new EmpresasService(pg as never, {} as never);

    const resultado = await servicio.actualizarPerfil('u-1', 'e-1', {
      background_alt: '  nuevo  ',
    });

    expect(fetchMock).not.toHaveBeenCalled();
    expect(resultado).toMatchObject({
      background_objeto_id: fondoId,
      background_media_type: 'video',
      background_alt: 'nuevo',
      background_activo: true,
    });
  });

  it('limpia todo el bloque con background_objeto_id: null sin validar storage', async () => {
    const fondoId = '22222222-3333-4444-8555-666666666666';
    const conFondo = {
      ...empresa,
      background_objeto_id: fondoId,
      background_media_type: 'imagen',
      background_poster_objeto_id: '33333333-4444-4555-8666-777777777777',
      background_alt: 'texto',
      background_overlay: 80,
      background_position: 'bottom' as const,
      background_activo: true,
    };
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce(conFondo)
        .mockResolvedValueOnce({ ok: 1 })
        .mockResolvedValueOnce(empresa),
    };
    const servicio = new EmpresasService(pg as never, {} as never);

    const resultado = await servicio.actualizarPerfil('u-1', 'e-1', {
      background_objeto_id: null,
    });

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

  it('rechaza activar sin fondo y un poster de video', async () => {
    const posterId = '33333333-4444-4555-8666-777777777777';
    const crearServicio = (actual = empresa) =>
      new EmpresasService(
        {
          queryOne: vi.fn().mockResolvedValueOnce(actual).mockResolvedValueOnce({ ok: 1 }),
        } as never,
        {} as never,
      );

    await expect(
      crearServicio().actualizarPerfil('u-1', 'e-1', { background_activo: true }),
    ).rejects.toMatchObject({ codigo: 'BACKGROUND_INVALIDO' });

    const conFondo = {
      ...empresa,
      background_objeto_id: '22222222-3333-4444-8555-666666666666',
      background_media_type: 'imagen' as const,
    };
    vi.stubGlobal('fetch', vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({
        tenant_id: 'e-1',
        estado: 'activo',
        publico: true,
        content_type: 'video/mp4',
      }),
    })));
    await expect(
      crearServicio(conFondo).actualizarPerfil('u-1', 'e-1', {
        background_poster_objeto_id: posterId,
      }),
    ).rejects.toMatchObject({ codigo: 'BACKGROUND_INVALIDO' });
  });

  it('rechaza logos inexistentes, no verificables o no públicos', async () => {
    const crearServicio = () =>
      new EmpresasService(
        {
          queryOne: vi.fn()
            .mockResolvedValueOnce(empresa)
            .mockResolvedValueOnce({ ok: 1 }),
        } as never,
        {} as never,
      );

    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 404 })));
    await expect(
      crearServicio().actualizarPerfil('u-1', 'e-1', { logo_objeto_id: logoId }),
    ).rejects.toBeInstanceOf(NotFoundError);

    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 500 })));
    await expect(
      crearServicio().actualizarPerfil('u-1', 'e-1', { logo_objeto_id: logoId }),
    ).rejects.toMatchObject({ codigo: 'LOGO_NO_VERIFICABLE' });

    vi.stubGlobal('fetch', vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({
        tenant_id: 'e-1',
        estado: 'activo',
        publico: false,
        content_type: 'image/png',
      }),
    })));
    await expect(
      crearServicio().actualizarPerfil('u-1', 'e-1', { logo_objeto_id: logoId }),
    ).rejects.toMatchObject({ codigo: 'LOGO_INVALIDO' });

    vi.stubGlobal('fetch', vi.fn(async () => {
      throw new Error('caído');
    }));
    await expect(
      crearServicio().actualizarPerfil('u-1', 'e-1', { logo_objeto_id: logoId }),
    ).rejects.toMatchObject({ codigo: 'LOGO_NO_VERIFICABLE' });
  });
});

describe('EmpresasService · actualizarPack', () => {
  const empresaConPack = {
    ...empresa,
    vertical_id: 'belleza',
    pack_codigo: 'pack_belleza',
    pack_version: 1,
  };

  it('devuelve la empresa sin tocar nada si el pack ya coincide', async () => {
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [empresaConPack], rowCount: 1 })
        .mockResolvedValueOnce({ rows: [{ codigo: 'pack_belleza', version: 1, vertical_codigo: 'belleza' }], rowCount: 1 }),
    };
    const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
    const servicio = new EmpresasService(pg as never, {} as never);

    await expect(servicio.actualizarPack('e-1', 'pack_belleza', 1)).resolves.toMatchObject({
      id: 'e-1',
      pack_version: 1,
    });
    expect(client.query).toHaveBeenCalledTimes(2);
  });

  it('reconcilia capacidades y audita el cambio de versión', async () => {
    const actualizada = { ...empresaConPack, pack_version: 2 };
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [empresaConPack], rowCount: 1 }) // lock empresa
        .mockResolvedValueOnce({ rows: [{ codigo: 'pack_belleza', version: 2, vertical_codigo: 'belleza' }], rowCount: 1 }) // pack
        .mockResolvedValueOnce({ rows: [{ total: 2 }], rowCount: 1 }) // conteo previo
        .mockResolvedValueOnce({ rows: [{ capacidad_codigo: 'catalogo_items', config: {}, activo: true }], rowCount: 1 }) // nuevas
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }) // delete sobrantes
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }) // upsert capacidad
        .mockResolvedValueOnce({ rows: [actualizada], rowCount: 1 }) // update empresa
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }), // auditoría
    };
    const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
    const servicio = new EmpresasService(pg as never, {} as never);

    const resultado = await servicio.actualizarPack('e-1', 'pack_belleza', 2);
    expect(resultado.pack_version).toBe(2);
    expect(client.query).toHaveBeenCalledWith(expect.stringContaining('DELETE FROM identity.empresa_capacidades'), [
      'e-1',
      ['catalogo_items'],
    ]);
    expect(client.query).toHaveBeenLastCalledWith(
      expect.stringContaining('INSERT INTO identity.pack_migraciones'),
      expect.arrayContaining(['e-1', 'belleza', 'pack_belleza', 2, 2, 1]),
    );
  });
});
