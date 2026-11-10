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
  parent_item_id: null,
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

  it('incluye parent_item_id e imágenes del item', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ n: 1 }).mockResolvedValueOnce(fila);
    (pg.query as Mock)
      .mockResolvedValueOnce([fila])
      .mockResolvedValueOnce([
        {
          id: 'img-1',
          item_id: fila.item_id,
          objeto_id: '33333333-4444-4555-8666-777777777777',
          orden: 0,
          es_principal: true,
        },
      ]);

    const pagina = await servicio.listar({ empresa_id: EMPRESA });

    expect(pagina.items[0]).toMatchObject({
      parent_item_id: null,
      imagenes: [
        {
          id: 'img-1',
          objeto_id: '33333333-4444-4555-8666-777777777777',
          orden: 0,
          es_principal: true,
        },
      ],
    });
  });

  it('crea productos y servicios desde la superficie unificada', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ id: 'padre-1', tipo: 'producto', empresa_id: EMPRESA })
      .mockResolvedValueOnce({ esquema: { type: 'object', additionalProperties: true } })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ ...fila, tipo: 'producto', parent_item_id: 'padre-1', track_inventory: true, stock: 4 });

    const item = await servicio.crearItem({
      tipo: 'producto',
      codigo: 'SKU-001',
      nombre: 'Teclado',
      precio: '100.00',
      stock: 4,
      parent_item_id: 'padre-1',
    }, EMPRESA);

    expect(item).toMatchObject({ tipo: 'producto', parent_item_id: 'padre-1', track_inventory: true, stock: 4 });
    expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('INSERT INTO catalog.items'), expect.any(Array));
  });

  it('crea servicios y alquileres válidos y rechaza un padre inválido', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ esquema: { type: 'object', additionalProperties: true } })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ ...fila, tipo: 'servicio', duration_minutes: 60, track_inventory: false, stock: null });
    await expect(servicio.crearItem({
      tipo: 'servicio', codigo: 'SERV-001', nombre: 'Consulta', precio: '10.00', duration_minutes: 60,
    }, EMPRESA)).resolves.toMatchObject({ tipo: 'servicio', duration_minutes: 60 });

    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ esquema: { type: 'object', additionalProperties: true } })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ ...fila, tipo: 'alquiler', track_inventory: false, stock: null });
    await expect(servicio.crearItem({
      tipo: 'alquiler', codigo: 'ALQ-001', nombre: 'Andamio', precio: '10.00',
    }, EMPRESA)).resolves.toMatchObject({ tipo: 'alquiler', track_inventory: false });

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.crearItem({
      tipo: 'producto', codigo: 'SKU-002', nombre: 'Padre', precio: '10.00', stock: 1, parent_item_id: '00000000-0000-4000-8000-000000000000',
    }, EMPRESA)).rejects.toMatchObject({ codigo: 'ITEM_PADRE_INVALIDO' });
  });

  it('exige duración en servicios y stock en productos inventariables', async () => {
    const { servicio } = crear();
    await expect(servicio.crearItem({
      tipo: 'servicio', codigo: 'SERV-001', nombre: 'Consulta', precio: '10.00',
    }, EMPRESA)).rejects.toMatchObject({ codigo: 'DURACION_INVALIDA' });

    await expect(servicio.crearItem({
      tipo: 'producto', codigo: 'SKU-001', nombre: 'Teclado', precio: '10.00',
    }, EMPRESA)).rejects.toMatchObject({ codigo: 'STOCK_INVALIDO' });
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
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(null).mockResolvedValueOnce({
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
    expect((pg.queryOne as Mock).mock.calls.at(-1)?.[0]).toContain("INSERT INTO catalog.items");
  });

  it('normaliza defaults y permite alquilar inactivo', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(null).mockResolvedValueOnce({
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
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(null).mockResolvedValueOnce(null);
    await expect(servicio.crearAlquiler({ codigo: 'ALQ-01', nombre: 'Sala', precio: '10.00' }, EMPRESA))
      .rejects.toMatchObject({ codigo: 'ITEM_NO_CREADO' });
  });

  it('traduce un código repetido en la misma empresa a conflicto', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ id: 'item-existente' });
    await expect(
      servicio.crearItem({ tipo: 'producto', codigo: 'SKU-001', nombre: 'Teclado', precio: '10.00', stock: 1 }, EMPRESA),
    ).rejects.toMatchObject({ codigo: 'CONFLICTO' });
    expect((pg.queryOne as Mock).mock.calls.at(-1)?.[1]).toEqual([EMPRESA, 'producto', 'SKU-001']);
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
