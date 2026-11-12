import type { Mock } from 'vitest';
import { ConflictError, DomainError, NotFoundError } from '@core/shared';
import { PreciosService } from './precios.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const ITEM = '22222222-2222-4222-8222-222222222222';
const LISTA = '33333333-3333-4333-8333-333333333333';
const PRECIO = '44444444-4444-4444-8444-444444444444';

const item = { id: ITEM, precio_cents: 10000, tipo_precio: 'fijo' as const };
const lista = {
  id: LISTA,
  empresa_id: EMPRESA,
  codigo: 'mayorista',
  nombre: 'Mayorista',
  descripcion: '',
  moneda: 'NIO',
  segmento_codigo: 'mayorista',
  predeterminada: false,
  activo: true,
  creado_en: '2026-11-12T00:00:00.000Z',
  actualizado_en: '2026-11-12T00:00:00.000Z',
};
const escala = {
  id: PRECIO,
  empresa_id: EMPRESA,
  lista_id: null,
  item_id: ITEM,
  desde_cantidad: 10,
  hasta_cantidad: null,
  precio_cents: 8000,
  creado_en: '2026-11-12T00:00:00.000Z',
  actualizado_en: '2026-11-12T00:00:00.000Z',
};

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn(), transaccion: vi.fn() };
  pg.transaccion.mockImplementation(async (fn: (client: unknown) => unknown) =>
    fn({ query: vi.fn() }),
  );
  return { servicio: new PreciosService(pg as never), pg };
}

describe('PreciosService (Fase P1.1)', () => {
  describe('listas', () => {
    it('crea una lista predeterminada y desmarca la anterior', async () => {
      const { servicio, pg } = crear();
      const client = { query: vi.fn().mockResolvedValue({ rows: [lista] }) };
      (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

      await expect(
        servicio.crearLista(EMPRESA, {
          codigo: 'Mayorista',
          nombre: 'Mayorista',
          segmento_codigo: 'Mayorista',
          predeterminada: true,
        }),
      ).resolves.toEqual(lista);

      expect(client.query).toHaveBeenNthCalledWith(
        1,
        expect.stringContaining('SET predeterminada = false'),
        [EMPRESA],
      );
      expect(client.query).toHaveBeenNthCalledWith(
        2,
        expect.stringContaining('INSERT INTO catalog.listas_precio'),
        [EMPRESA, 'mayorista', 'Mayorista', '', 'NIO', 'mayorista', true],
      );
    });

    it('traduce duplicados a 409 y valida el codigo', async () => {
      const { servicio, pg } = crear();
      (pg.transaccion as Mock).mockRejectedValueOnce({ code: '23505' });
      await expect(
        servicio.crearLista(EMPRESA, { codigo: 'mayorista', nombre: 'Mayorista' }),
      ).rejects.toBeInstanceOf(ConflictError);

      await expect(
        servicio.crearLista(EMPRESA, { codigo: 'Mayorista!', nombre: 'Mayorista' }),
      ).rejects.toBeInstanceOf(DomainError);
    });

    it('desactiva la predeterminada al apagar la lista', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(lista);
      const fila = { ...lista, predeterminada: false, activo: false };
      const client = { query: vi.fn().mockResolvedValue({ rows: [fila] }) };
      (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

      await expect(
        servicio.actualizarLista(EMPRESA, LISTA, { activo: false, predeterminada: true }),
      ).resolves.toEqual(fila);
      expect(client.query).toHaveBeenCalledWith(
        expect.stringContaining('UPDATE catalog.listas_precio'),
        ['mayorista', 'Mayorista', '', 'NIO', 'mayorista', false, false, LISTA, EMPRESA],
      );
    });
  });

  describe('precios', () => {
    it('crea un precio de lista sin tocar tipo_precio del item', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(lista)
        .mockResolvedValueOnce(item)
        .mockResolvedValueOnce({ ...escala, lista_id: LISTA, desde_cantidad: 1 });

      await expect(
        servicio.crearPrecio(EMPRESA, LISTA, { item_id: ITEM, precio_cents: 9000 }),
      ).resolves.toMatchObject({ lista_id: LISTA });

      expect(pg.query).not.toHaveBeenCalledWith(
        expect.stringContaining("tipo_precio = 'escala'"),
        expect.anything(),
      );
    });

    it('crea una escala base y activa tipo_precio = escala', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(item).mockResolvedValueOnce(escala);

      await expect(
        servicio.crearPrecio(EMPRESA, null, {
          item_id: ITEM,
          desde_cantidad: 10,
          precio_cents: 8000,
        }),
      ).resolves.toEqual(escala);

      expect(pg.query).toHaveBeenCalledWith(
        expect.stringContaining("tipo_precio = 'escala'"),
        [ITEM, EMPRESA],
      );
    });

    it('traduce el solape de escalas a 409', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(item).mockRejectedValueOnce({ code: '23P01' });
      await expect(
        servicio.crearPrecio(EMPRESA, null, { item_id: ITEM, precio_cents: 8000 }),
      ).rejects.toBeInstanceOf(ConflictError);
    });

    it('valida el rango de cantidad y el precio', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(item);
      await expect(
        servicio.crearPrecio(EMPRESA, null, {
          item_id: ITEM,
          desde_cantidad: 10,
          hasta_cantidad: 5,
          precio_cents: 8000,
        }),
      ).rejects.toBeInstanceOf(DomainError);
      await expect(
        servicio.crearPrecio(EMPRESA, null, { item_id: ITEM, precio_cents: -1 }),
      ).rejects.toBeInstanceOf(DomainError);
    });

    it('recalcula tipo_precio al eliminar la ultima escala base', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(escala)
        .mockResolvedValueOnce({ id: PRECIO });

      await expect(servicio.eliminarPrecio(EMPRESA, null, PRECIO)).resolves.toEqual({ ok: true });
      expect(pg.query).toHaveBeenCalledWith(
        expect.stringContaining('SET tipo_precio = CASE WHEN EXISTS'),
        [ITEM, EMPRESA],
      );
    });

    it('exige que la escala pertenezca al item de la ruta', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(escala);
      await expect(
        servicio.actualizarEscala(EMPRESA, 'otro-item', PRECIO, { precio_cents: 7000 }),
      ).rejects.toBeInstanceOf(NotFoundError);
    });
  });


  it('lista listas y precios de una lista', async () => {
    const { servicio, pg } = crear();
    (pg.query as Mock).mockResolvedValueOnce([lista]);
    await expect(servicio.listarListas(EMPRESA)).resolves.toEqual([lista]);

    (pg.queryOne as Mock).mockResolvedValueOnce(lista);
    (pg.query as Mock).mockResolvedValueOnce([escala]);
    await expect(servicio.listarPrecios(EMPRESA, LISTA)).resolves.toEqual([escala]);
  });

  it('crea una lista no predeterminada sin tocar otras listas', async () => {
    const { servicio, pg } = crear();
    const client = { query: vi.fn().mockResolvedValue({ rows: [{ ...lista, predeterminada: false }] }) };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(
      servicio.crearLista(EMPRESA, { codigo: 'vip', nombre: 'VIP' }),
    ).resolves.toMatchObject({ predeterminada: false });
    expect(client.query).toHaveBeenCalledTimes(1);
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO catalog.listas_precio'),
      [EMPRESA, 'vip', 'VIP', '', 'NIO', '', false],
    );
  });

  it('valida nombre, moneda, segmento y uuid de lista', async () => {
    const { servicio } = crear();
    await expect(
      servicio.crearLista(EMPRESA, { codigo: 'vip', nombre: 'x' }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crearLista(EMPRESA, { codigo: 'vip', nombre: 'VIP', moneda: 'peso' }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crearLista(EMPRESA, { codigo: 'vip', nombre: 'VIP', segmento_codigo: 'V I P' }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(servicio.actualizarLista(EMPRESA, 'no-uuid', {})).rejects.toBeInstanceOf(DomainError);
  });

  it('actualiza una lista completa y traduce conflictos', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(lista);
    const client = { query: vi.fn().mockResolvedValue({ rows: [{ ...lista, nombre: 'VIP' }] }) };
    (pg.transaccion as Mock).mockImplementationOnce(async (fn: (c: unknown) => unknown) => fn(client));

    await expect(
      servicio.actualizarLista(EMPRESA, LISTA, {
        codigo: 'vip',
        nombre: 'VIP',
        descripcion: 'Segmento vip',
        moneda: 'usd',
        segmento_codigo: 'vip',
        predeterminada: true,
      }),
    ).resolves.toMatchObject({ nombre: 'VIP' });
    expect(client.query).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('AND id <> $2'),
      [EMPRESA, LISTA],
    );

    (pg.queryOne as Mock).mockResolvedValueOnce(lista);
    (pg.transaccion as Mock).mockRejectedValueOnce({ code: '23505' });
    await expect(
      servicio.actualizarLista(EMPRESA, LISTA, { nombre: 'Otro' }),
    ).rejects.toBeInstanceOf(ConflictError);
  });

  it('propaga errores no conflictivos y listas inexistentes', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(lista);
    (pg.transaccion as Mock).mockRejectedValueOnce(new Error('db caida'));
    await expect(servicio.actualizarLista(EMPRESA, LISTA, {})).rejects.toThrow('db caida');

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.listarPrecios(EMPRESA, LISTA)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('actualiza un precio con campos totales o parciales y valida errores', async () => {
    const { servicio, pg } = crear();
    const actualizado = { ...escala, desde_cantidad: 1, hasta_cantidad: 9, precio_cents: 9000 };
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(escala)
      .mockResolvedValueOnce(actualizado);
    await expect(
      servicio.actualizarPrecio(EMPRESA, null, PRECIO, {
        desde_cantidad: 1,
        hasta_cantidad: 9,
        precio_cents: 9000,
      }),
    ).resolves.toEqual(actualizado);

    (pg.queryOne as Mock).mockResolvedValueOnce(escala).mockResolvedValueOnce(null);
    await expect(servicio.actualizarPrecio(EMPRESA, null, PRECIO, {})).rejects.toBeInstanceOf(
      NotFoundError,
    );

    (pg.queryOne as Mock).mockResolvedValueOnce(escala).mockRejectedValueOnce({ code: '23P01' });
    await expect(servicio.actualizarPrecio(EMPRESA, null, PRECIO, {})).rejects.toBeInstanceOf(
      ConflictError,
    );
  });

  it('lista escalas y actualiza/elimina una escala existente', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    (pg.query as Mock).mockResolvedValueOnce([escala]);
    await expect(servicio.listarEscalas(EMPRESA, ITEM)).resolves.toEqual([escala]);

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(escala)
      .mockResolvedValueOnce(escala)
      .mockResolvedValue({ ...escala, precio_cents: 7500 });
    await expect(
      servicio.actualizarEscala(EMPRESA, ITEM, PRECIO, { precio_cents: 7500 }),
    ).resolves.toMatchObject({ precio_cents: 7500 });

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(escala)
      .mockResolvedValueOnce(escala)
      .mockResolvedValueOnce({ id: PRECIO });
    await expect(servicio.eliminarEscala(EMPRESA, ITEM, PRECIO)).resolves.toEqual({ ok: true });

    (pg.queryOne as Mock).mockResolvedValueOnce(escala);
    await expect(
      servicio.eliminarEscala(EMPRESA, 'otro-item', PRECIO),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it('falla cerrado al pedir una lista inexistente o un codigo invalido', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item).mockResolvedValueOnce(null);
    await expect(
      servicio.resolver(EMPRESA, ITEM, 1, { lista_codigo: 'no_existe' }),
    ).rejects.toBeInstanceOf(NotFoundError);

    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    await expect(
      servicio.resolver(EMPRESA, ITEM, 1, { lista_codigo: 'X!' }),
    ).rejects.toBeInstanceOf(DomainError);
  });

  it('rechaza un total que excede el rango seguro', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ ...item, precio_cents: Number.MAX_SAFE_INTEGER })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null);
    await expect(servicio.resolver(EMPRESA, ITEM, 2)).rejects.toBeInstanceOf(DomainError);
  });

  it('rechaza la insercion de un precio nulo', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item).mockResolvedValueOnce(null);
    await expect(
      servicio.crearPrecio(EMPRESA, null, { item_id: ITEM, precio_cents: 100 }),
    ).rejects.toBeInstanceOf(DomainError);
  });

  describe('resolucion', () => {
    it('usa la escala de una lista explicita', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(item)
        .mockResolvedValueOnce(lista)
        .mockResolvedValueOnce({ ...escala, lista_id: LISTA, desde_cantidad: 10, precio_cents: 7000 });

      await expect(servicio.resolver(EMPRESA, ITEM, '12', { lista_codigo: 'Mayorista' }))
        .resolves.toMatchObject({
          origen: 'lista',
          lista: { id: LISTA, codigo: 'mayorista' },
          precio_unitario_cents: 7000,
          precio_total_cents: 84000,
        });
      expect(pg.queryOne).toHaveBeenNthCalledWith(
        2,
        expect.stringContaining('lower(codigo) = $2'),
        [EMPRESA, 'mayorista'],
      );
    });

    it('usa la lista predeterminada y cae al precio base sin escala', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(item)
        .mockResolvedValueOnce({ ...lista, predeterminada: true })
        .mockResolvedValueOnce(null);

      await expect(servicio.resolver(EMPRESA, ITEM, 3)).resolves.toMatchObject({
        origen: 'base',
        precio_unitario_cents: 10000,
        precio_total_cents: 30000,
      });
    });

    it('resuelve la escala base cuando no hay lista', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(item)
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(escala);

      await expect(servicio.resolver(EMPRESA, ITEM, 10)).resolves.toMatchObject({
        origen: 'escala',
        escala: { desde_cantidad: 10 },
        precio_unitario_cents: 8000,
      });
    });

    it('resuelve por segmento y falla cerrado si no existe la lista', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(item)
        .mockResolvedValueOnce(lista)
        .mockResolvedValueOnce(null);

      await servicio.resolver(EMPRESA, ITEM, 2, { segmento_codigo: 'Mayorista' });
      expect(pg.queryOne).toHaveBeenNthCalledWith(
        2,
        expect.stringContaining('segmento_codigo = $2'),
        [EMPRESA, 'mayorista'],
      );

      (pg.queryOne as Mock).mockResolvedValueOnce(item).mockResolvedValueOnce(null);
      await expect(
        servicio.resolver(EMPRESA, ITEM, 2, { segmento_codigo: 'sin_lista' }),
      ).rejects.toBeInstanceOf(NotFoundError);
    });

    it('rechaza cantidades invalidas antes de consultar', async () => {
      const { servicio, pg } = crear();
      await expect(servicio.resolver(EMPRESA, ITEM, 0)).rejects.toBeInstanceOf(DomainError);
      await expect(servicio.resolver(EMPRESA, ITEM, '1.5')).rejects.toBeInstanceOf(DomainError);
      expect(pg.queryOne).not.toHaveBeenCalled();
    });
  });
});
