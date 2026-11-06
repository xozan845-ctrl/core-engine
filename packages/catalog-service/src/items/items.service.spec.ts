import type { Mock } from 'vitest';
import { ItemsService } from './items.service';

const EMPRESA = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';

function crear() {
  const pg = {
    query: vi.fn().mockResolvedValue([]),
    queryOne: vi.fn().mockResolvedValue(null),
  };
  return { pg, servicio: new ItemsService(pg as never) };
}

const fila = {
  item_id: '11111111-2222-3333-4444-555555555555',
  empresa_id: EMPRESA,
  tipo: 'servicio' as const,
  codigo: 'SERV-001',
  nombre: 'Consulta',
  descripcion: '',
  categoria: 'salud',
  precio_cents: 150000,
  costo_cents: 50000,
  track_inventory: false,
  stock: null,
  duration_minutes: 60,
  atributos: { especialidad: 'general' },
  estado: 'activo',
  source_type: 'servicio' as const,
  source_id: '22222222-3333-4444-5555-666666666666',
  creado_en: '2026-11-01T00:00:00.000Z',
  actualizado_en: '2026-11-01T00:00:00.000Z',
};

describe('ItemsService (fase 2)', () => {
  it('lista items unificados y serializa dinero y nullables', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ n: 1 }).mockResolvedValueOnce(fila);
    (pg.query as Mock).mockResolvedValue([fila]);

    const pagina = await servicio.listar({ empresa_id: EMPRESA, tipo: 'servicio' });

    expect(pagina.items[0]).toMatchObject({
      item_id: fila.item_id,
      tipo: 'servicio',
      precio: '1500.00',
      costo: '500.00',
      stock: null,
      duration_minutes: 60,
    });
  });

  it('aplica filtros opcionales, incluye inactivos y valida ids', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue({ n: 0 });
    await servicio.listar({
      empresa_id: EMPRESA,
      tipo: 'producto',
      categoria: 'farmacia',
      q: 'aspirina',
      incluir_inactivos: true,
    });
    expect((pg.query as Mock).mock.calls[0][1]).toEqual([
      EMPRESA,
      'producto',
      'farmacia',
      '%aspirina%',
      20,
      0,
    ]);
    await expect(servicio.listar({ empresa_id: 'not-uuid' })).rejects.toMatchObject({ codigo: 'EMPRESA_REQUERIDA' });
    await expect(servicio.listar({ empresa_id: EMPRESA, tipo: 'otro' })).rejects.toMatchObject({ codigo: 'TIPO_ITEM_INVALIDO' });
    await expect(servicio.encontrarPorId('not-uuid')).resolves.toBeNull();
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.encontrarPorId(fila.item_id)).resolves.toBeNull();
    await expect(servicio.esquemas()).resolves.toEqual([]);
  });

  it('lista sin filtros de tipo y consulta un item por empresa', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ n: 1 }).mockResolvedValueOnce(fila);
    (pg.query as Mock).mockResolvedValue([fila]);
    await expect(servicio.listar({ empresa_id: EMPRESA })).resolves.toMatchObject({ total: 1 });
    await expect(servicio.encontrarPorId(fila.item_id, EMPRESA)).resolves.toMatchObject({ item_id: fila.item_id });
    expect((pg.queryOne as Mock).mock.calls.at(-1)?.[1]).toEqual([fila.item_id, EMPRESA]);
  });

  it('rechaza atributos que no cumplen el schema de la categoría', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue({
      esquema: {
        type: 'object',
        required: ['especialidad'],
        properties: { especialidad: { type: 'string' } },
        additionalProperties: false,
      },
    });

    await expect(
      servicio.validarAtributos(EMPRESA, 'servicio', 'salud', { color: 'rojo' }),
    ).rejects.toMatchObject({ codigo: 'ATRIBUTOS_INVALIDOS' });
  });

  it('crea un alquiler como item sin inventario', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce({
      ...fila,
      tipo: 'alquiler',
      source_type: null,
      source_id: null,
      codigo: 'ALQ-001',
      nombre: 'Sala',
      precio_cents: 250000,
      costo_cents: 0,
      duration_minutes: null,
      atributos: { capacidad: 8 },
    });

    const resultado = await servicio.crearAlquiler(
      { codigo: 'alq-001', nombre: 'Sala', precio: '2500.00', atributos: { capacidad: 8 } },
      EMPRESA,
    );

    expect(resultado).toMatchObject({ tipo: 'alquiler', codigo: 'ALQ-001', track_inventory: false });
    expect((pg.queryOne as Mock).mock.calls[1][0]).toContain("INSERT INTO catalog.items");
  });

  it('normaliza defaults y permite alquilar inactivo', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce({
      ...fila,
      tipo: 'alquiler',
      source_type: null,
      source_id: null,
      codigo: 'ALQ-002',
      nombre: 'Sala inactiva',
      estado: 'inactivo',
      atributos: {},
    });
    await expect(servicio.crearAlquiler({ codigo: ' alq-002 ', nombre: ' Sala inactiva ', precio: 10, estado: 'inactivo' }, EMPRESA))
      .resolves.toMatchObject({ estado: 'inactivo', codigo: 'ALQ-002' });
  });

  it('maneja esquema ausente, entradas inválidas y alquiler no creado', async () => {
    const { servicio, pg } = crear();
    await expect(servicio.validarAtributos(EMPRESA, 'producto', 'general', [])).rejects.toMatchObject({ codigo: 'ATRIBUTOS_INVALIDOS' });
    await expect(servicio.validarAtributos(EMPRESA, 'producto', 'general', { ok: true })).resolves.toEqual({ ok: true });
    await expect(servicio.guardarEsquema(EMPRESA, 'otro', 'x', {})).rejects.toMatchObject({ codigo: 'TIPO_ITEM_INVALIDO' });
    await expect(servicio.guardarEsquema(EMPRESA, 'producto', ' ', {})).rejects.toMatchObject({ codigo: 'CATEGORIA_INVALIDA' });
    await expect(servicio.guardarEsquema(EMPRESA, 'producto', 'x', { type: 'wat' })).rejects.toMatchObject({ codigo: 'ESQUEMA_INVALIDO' });
    await expect(servicio.guardarEsquema(EMPRESA, 'producto', 'x', [] as never)).rejects.toMatchObject({ codigo: 'ESQUEMA_INVALIDO' });
    await expect(servicio.guardarEsquema(EMPRESA, 'producto', 'x', null as never)).rejects.toMatchObject({ codigo: 'ESQUEMA_INVALIDO' });
    await expect(servicio.crearAlquiler({ codigo: 'A', nombre: 'Sala', precio: '10.00' }, EMPRESA))
      .rejects.toMatchObject({ codigo: 'CODIGO_INVALIDO' });
    await expect(servicio.crearAlquiler({ codigo: 'ALQ-01', nombre: 'S', precio: '10.00' }, EMPRESA))
      .rejects.toMatchObject({ codigo: 'NOMBRE_INVALIDO' });
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(null);
    await expect(servicio.crearAlquiler({ codigo: 'ALQ-01', nombre: 'Sala', precio: '10.00' }, EMPRESA))
      .rejects.toMatchObject({ codigo: 'ITEM_NO_CREADO' });
  });

  it('valida y registra esquemas JSON', async () => {
    const { servicio, pg } = crear();
    await servicio.guardarEsquema(EMPRESA, 'producto', 'farmacia', {
      type: 'object',
      properties: { principio_activo: { type: 'string' } },
      additionalProperties: false,
    });
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('item_atributo_esquemas'), expect.any(Array));
  });
});
