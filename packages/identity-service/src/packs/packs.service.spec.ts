import { NotFoundError } from '@core/shared';
import { PacksService } from './packs.service';

const fila = {
  codigo: 'pack_belleza',
  version: 1,
  vertical_codigo: 'belleza',
  nombre: 'Belleza',
  descripcion: 'Servicios agendados',
  config: { modelo: 'servicios_agendados' },
  capacidad_codigo: 'agenda_recursos',
  capacidad_nombre: 'Agenda y recursos',
  capacidad_modulo: 'scheduling',
  capacidad_activo: true,
  capacidad_config: {},
};

describe('PacksService', () => {
  it('debe agrupar las capacidades de los packs cuando se listan por vertical', async () => {
    const pg = { query: vi.fn().mockResolvedValue([fila, { ...fila, capacidad_codigo: 'facturacion', capacidad_nombre: 'Facturacion', capacidad_modulo: 'finance' }]) };
    const servicio = new PacksService(pg as never);

    await expect(servicio.listar('belleza')).resolves.toEqual([{
      codigo: 'pack_belleza', version: 1, vertical_codigo: 'belleza', nombre: 'Belleza',
      descripcion: 'Servicios agendados', config: { modelo: 'servicios_agendados' },
      modelos: [],
      capacidades: [
        { capacidad_codigo: 'agenda_recursos', nombre: 'Agenda y recursos', modulo: 'scheduling', activo: true, config: {} },
        { capacidad_codigo: 'facturacion', nombre: 'Facturacion', modulo: 'finance', activo: true, config: {} },
      ],
    }]);
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('p.vertical_codigo = $1'), ['belleza']);
  });

  it('debe devolver un pack versionado', async () => {
    const pg = { query: vi.fn().mockResolvedValue([fila]) };
    await expect(new PacksService(pg as never).obtener('pack_belleza', 1)).resolves.toMatchObject({ codigo: 'pack_belleza', version: 1 });
  });

  it('debe lanzar NotFoundError cuando el pack no existe', async () => {
    const pg = { query: vi.fn().mockResolvedValue([]) };
    await expect(new PacksService(pg as never).obtener('pack_no', 1)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe usar los fallbacks de capacidad y el default de versión', async () => {
    const pg = {
      query: vi.fn().mockResolvedValue([
        { ...fila, capacidad_codigo: 'sin_metadata', capacidad_nombre: null, capacidad_modulo: null, capacidad_activo: null, capacidad_config: null },
        { ...fila, codigo: 'pack_vacio', capacidad_codigo: null },
      ]),
    };
    const servicio = new PacksService(pg as never);

    const packs = await servicio.obtener('pack_belleza');
    expect(packs.capacidades[0]).toMatchObject({
      nombre: 'sin_metadata',
      modulo: 'shared',
      activo: false,
      config: {},
    });
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('p.version = $2'), ['pack_belleza', 1]);
  });

  it('debe listar modelos y verticales activos', async () => {
    const pg = {
      query: vi.fn()
        .mockResolvedValueOnce([{ codigo: 'modelo' }])
        .mockResolvedValueOnce([{ codigo: 'vertical' }]),
    };
    const servicio = new PacksService(pg as never);

    await expect(servicio.listarModelos()).resolves.toEqual([{ codigo: 'modelo' }]);
    await expect(servicio.listarVerticales()).resolves.toEqual([{ codigo: 'vertical' }]);
  });

  it('debe crear modelos operativos válidos y rechazar nombres/códigos inválidos', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue({ codigo: 'modelo' }) };
    const servicio = new PacksService(pg as never);

    await expect(
      servicio.crearModelo({ codigo: ' Modelo ', nombre: ' Modelo ', descripcion: ' d ', config: { a: 1 } }),
    ).resolves.toMatchObject({ codigo: 'modelo' });
    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining('identity.modelos_operacion'),
      ['modelo', 'Modelo', 'd', JSON.stringify({ a: 1 }), true],
    );
    await expect(servicio.crearModelo({ codigo: '1MAL', nombre: 'Modelo' })).rejects.toThrow(
      /caracteres no permitidos/,
    );
    await expect(servicio.crearModelo({ codigo: 'modelo', nombre: 'x' })).rejects.toThrow(
      /nombre del modelo/,
    );
  });

  it('debe crear verticales válidas, validar el modelo y rechazar datos inválidos', async () => {
    const pg = { queryOne: vi.fn() };
    pg.queryOne
      .mockResolvedValueOnce({ codigo: 'modelo' })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ codigo: 'modelo' })
      .mockResolvedValueOnce({ codigo: 'vertical' });
    const servicio = new PacksService(pg as never);

    await expect(
      servicio.crearVertical({ codigo: 'vertical', nombre: 'Vertical', modelo_operacion: 'modelo' }),
    ).rejects.toThrow(/No se pudo crear la vertical/);
    await expect(
      servicio.crearVertical({ codigo: 'vertical', nombre: 'Vertical', modelo_operacion: 'modelo' }),
    ).resolves.toMatchObject({ codigo: 'vertical' });
    await expect(
      servicio.crearVertical({ codigo: 'vertical', nombre: 'x', modelo_operacion: 'modelo' }),
    ).rejects.toThrow(/datos de la vertical/);

    pg.queryOne.mockResolvedValueOnce(null);
    await expect(
      servicio.crearVertical({ codigo: 'vertical', nombre: 'Vertical', modelo_operacion: 'inexistente' }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe rechazar packs con código, versión o nombre inválidos', async () => {
    const servicio = new PacksService({} as never);

    await expect(
      servicio.crear({ codigo: 'pack_', version: 1, vertical_codigo: 'vertical', nombre: 'Pack' }),
    ).rejects.toThrow(/empezar por pack_/);
    await expect(
      servicio.crear({ codigo: 'pack_xy', version: 0, vertical_codigo: 'vertical', nombre: 'Pack' }),
    ).rejects.toThrow(/entero positivo/);
    await expect(
      servicio.crear({ codigo: 'pack_xy', version: 1, vertical_codigo: 'vertical', nombre: 'x' }),
    ).rejects.toThrow(/nombre del pack/);
  });

  it('debe fallar si la vertical, un modelo o una capacidad no existen', async () => {
    const conRespuestas = (...rowCounts: number[]) => {
      const query = vi.fn();
      for (const rowCount of rowCounts) {
        query.mockResolvedValueOnce({ rowCount, rows: rowCount ? [{}] : [] });
      }
      return new PacksService({
        transaccion: vi.fn(async (fn: (c: unknown) => unknown) => fn({ query })),
      } as never);
    };

    await expect(
      conRespuestas(0).crear({ codigo: 'pack_xy', version: 1, vertical_codigo: 'no_existe', nombre: 'Pack' }),
    ).rejects.toBeInstanceOf(NotFoundError);
    await expect(
      conRespuestas(1, 1, 0).crear({
        codigo: 'pack_xy', version: 1, vertical_codigo: 'vertical', nombre: 'Pack',
        modelos: [{ codigo: 'modelo' }],
      }),
    ).rejects.toBeInstanceOf(NotFoundError);
    await expect(
      conRespuestas(1, 1, 0).crear({
        codigo: 'pack_xy', version: 1, vertical_codigo: 'vertical', nombre: 'Pack',
        capacidades: [{ codigo: 'capacidad' }],
      }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe construir un pack dinámico con modelos y capacidades existentes', async () => {
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rowCount: 1, rows: [{ codigo: 'servicios_domicilio' }] })
        .mockResolvedValueOnce({ rowCount: 1, rows: [] })
        .mockResolvedValueOnce({ rowCount: 1, rows: [] })
        .mockResolvedValueOnce({ rowCount: 1, rows: [] })
        .mockResolvedValueOnce({ rowCount: 1, rows: [] }),
    };
    const resultado = {
      ...fila,
      codigo: 'pack_servicios_domicilio',
      vertical_codigo: 'servicios_domicilio',
      modelos: [{ modelo_codigo: 'servicios_agendados', nombre: 'Servicios agendados', descripcion: '', activo: true, config: {} }],
    };
    const pg = {
      transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)),
      query: vi.fn().mockResolvedValue([resultado]),
    };
    const servicio = new PacksService(pg as never);

    await expect(servicio.crear({
      codigo: 'pack_servicios_domicilio',
      version: 1,
      vertical_codigo: 'servicios_domicilio',
      nombre: 'Servicios a domicilio',
      modelos: [{ codigo: 'servicios_agendados' }],
      capacidades: [{ codigo: 'agenda_recursos' }],
    })).resolves.toMatchObject({
      codigo: 'pack_servicios_domicilio',
      modelos: [{ modelo_codigo: 'servicios_agendados' }],
    });
    expect(client.query).toHaveBeenCalledTimes(5);
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO identity.vertical_capacidades'),
      ['servicios_domicilio', 'pack_servicios_domicilio', 1],
    );
  });
});
