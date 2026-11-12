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
