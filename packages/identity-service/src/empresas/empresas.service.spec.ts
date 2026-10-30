import type { Mock } from 'vitest';
import { ConflictError, ForbiddenError, NotFoundError } from '@core/shared';
import { EmpresasService } from './empresas.service';

const empresa = {
  id: 'e-1',
  nombre: 'Acme',
  slug: 'acme',
  modalidad_logistica: 'hibrida' as const,
  estado: 'activa',
  creado_por: 'u-1',
  creado_en: '2026-10-23T00:00:00.000Z',
};

function clienteInsertable(): { query: Mock } {
  return {
    query: vi.fn()
      .mockResolvedValueOnce({ rows: [], rowCount: 0 }) // slug libre
      .mockResolvedValueOnce({ rows: [empresa], rowCount: 1 }) // INSERT empresa
      .mockResolvedValueOnce({ rows: [], rowCount: 1 }) // INSERT miembro
      .mockResolvedValueOnce({ rows: [], rowCount: 1 }), // UPDATE usuario
  };
}

describe('EmpresasService', () => {
  describe('crear (ADR-26)', () => {
    it('deriva el slug del nombre, crea la empresa y activa el tenant', async () => {
      const client = clienteInsertable();
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.crear('u-1', '  Acme  ', 'hibrida')).resolves.toEqual(empresa);

      expect(client.query).toHaveBeenNthCalledWith(
        1,
        expect.stringContaining('FROM identity.empresa_slugs'),
        ['acme'],
      );
      expect(client.query).toHaveBeenNthCalledWith(
        2,
        expect.stringContaining('INSERT INTO identity.empresas'),
        expect.arrayContaining(['Acme', 'acme', 'hibrida', 'u-1']),
      );
      expect(client.query).toHaveBeenCalledTimes(4);
    });

    it('resuelve una colisión de slug automático con sufijo -2', async () => {
      const client = {
        query: vi.fn()
          .mockResolvedValueOnce({ rows: [{ ok: 1 }], rowCount: 1 }) // acme ocupado
          .mockResolvedValueOnce({ rows: [], rowCount: 0 }) // acme-2 libre
          .mockResolvedValueOnce({ rows: [empresa], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 }),
      };
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await servicio.crear('u-1', 'Acme', 'hibrida');

      expect(client.query).toHaveBeenNthCalledWith(2, expect.any(String), ['acme-2']);
      expect(client.query).toHaveBeenNthCalledWith(
        3,
        expect.stringContaining('INSERT INTO identity.empresas'),
        expect.arrayContaining(['Acme', 'acme-2', 'hibrida', 'u-1']),
      );
    });

    it('acepta un slug explícito normalizado', async () => {
      const client = {
        query: vi.fn()
          .mockResolvedValueOnce({ rows: [], rowCount: 0 })
          .mockResolvedValueOnce({ rows: [empresa], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 })
          .mockResolvedValueOnce({ rows: [], rowCount: 1 }),
      };
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await servicio.crear('u-1', 'Acme', 'hibrida', '  Mi Empresa  ');

      expect(client.query).toHaveBeenNthCalledWith(1, expect.any(String), ['mi-empresa']);
      expect(client.query).toHaveBeenNthCalledWith(
        2,
        expect.stringContaining('INSERT INTO identity.empresas'),
        expect.arrayContaining(['Acme', 'mi-empresa', 'hibrida', 'u-1']),
      );
    });

    it('rechaza con 409 un slug explícito ya usado (incluido histórico)', async () => {
      const client = { query: vi.fn().mockResolvedValueOnce({ rows: [{ ok: 1 }], rowCount: 1 }) };
      const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.crear('u-1', 'Acme', 'hibrida', 'acme')).rejects.toBeInstanceOf(
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

      await expect(servicio.crear('u-1', 'Acme', 'hibrida')).resolves.toEqual(empresa);
      expect(pg.transaccion).toHaveBeenCalledTimes(2);
    });

    it('traduce una violación UNIQUE de un slug explícito a ConflictError', async () => {
      const pg = { transaccion: vi.fn().mockRejectedValue({ code: '23505' }) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.crear('u-1', 'Acme', 'hibrida', 'acme')).rejects.toBeInstanceOf(
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

      await expect(servicio.crear('u-1', 'Acme', 'hibrida')).resolves.toEqual(empresa);

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

  it('actualiza la modalidad cuando el actor es empresario activo', async () => {
    const actualizada = { ...empresa, modalidad_logistica: 'propia' };
    const pg = { queryOne: vi.fn().mockResolvedValueOnce({ ok: 1 }).mockResolvedValueOnce(actualizada) };
    const servicio = new EmpresasService(pg as never, {} as never);

    await expect(servicio.actualizarModalidad('u-1', 'e-1', 'propia')).resolves.toEqual(actualizada);
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      2, expect.stringContaining('UPDATE identity.empresas'), ['propia', 'e-1'],
    );
  });

  it('rechaza a quien no es empresario activo de la empresa', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue(null) };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(servicio.actualizarModalidad('u-x', 'e-1', 'hibrida'))
      .rejects.toBeInstanceOf(ForbiddenError);
  });

  it('no modifica una empresa inactiva o inexistente', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValueOnce({ ok: 1 }).mockResolvedValueOnce(null) };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(servicio.actualizarModalidad('u-1', 'e-1', 'plataforma'))
      .rejects.toBeInstanceOf(NotFoundError);
  });

  describe('resolverPara (UUID o slug, ADR-26)', () => {
    it('resuelve un UUID como administrador global sin consultar membresía', async () => {
      const pg = { queryOne: vi.fn().mockResolvedValue({ ...empresa, es_actual: true }) };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.resolverPara('sa-1', 'e-1', true)).resolves.toEqual({ empresa });
      expect(pg.queryOne).toHaveBeenCalledTimes(1);
    });

    it('resuelve el slug actual para un miembro activo', async () => {
      const pg = {
        queryOne: vi.fn()
          .mockResolvedValueOnce({ ...empresa, es_actual: true })
          .mockResolvedValueOnce({ ok: 1 }),
      };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.resolverPara('u-1', 'acme', false)).resolves.toEqual({ empresa });
      expect(pg.queryOne).toHaveBeenNthCalledWith(1, expect.stringContaining('identity.empresa_slugs'), ['acme']);
    });

    it('devuelve la ruta actual cuando el slug consultado es anterior', async () => {
      const pg = {
        queryOne: vi.fn()
          .mockResolvedValueOnce({ ...empresa, slug: 'acme-nueva', es_actual: false })
          .mockResolvedValueOnce({ ok: 1 }),
      };
      const servicio = new EmpresasService(pg as never, {} as never);

      await expect(servicio.resolverPara('u-1', 'acme-vieja', false)).resolves.toEqual({
        empresa: { ...empresa, slug: 'acme-nueva' },
        redirigirA: '/api/v1/empresas/acme-nueva',
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
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({
          id: logoId,
          tenant_id: 'e-1',
          estado: 'activo',
          publico: true,
          content_type: 'image/png',
        }),
      })),
    );
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

    expect(resultado.logo_url).toBe(`/api/v1/storage/public/objetos/${logoId}`);
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
