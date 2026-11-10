import type { Mock } from 'vitest';
import 'reflect-metadata';
import { CARRITO_EXPIRACION_MS, NotFoundError } from '@core/shared';
import type { PgService } from '@core/shared';
import {
  CANTIDAD_MAXIMA_POR_LINEA,
  CarritoService,
  type ItemCarrito,
} from './carrito.service';

type Oferta = {
  id: string;
  tienda_id: string;
  vendedor_id: string;
  sku: string;
  producto_nombre: string;
  precio_venta: string;
  stock: number;
  estado: string;
};

const ofertaActiva: Oferta = {
  id: 'of-1',
  tienda_id: 't-1',
  vendedor_id: 'v-1',
  sku: 'SKU-1',
  producto_nombre: 'Producto 1',
  precio_venta: '12.50',
  stock: 20,
  estado: 'activa',
};

const item: ItemCarrito = { oferta_id: 'of-1', cantidad: 2 };
const actualizado = '2026-01-01T00:00:00.000Z';

function respuestaOfertas(ofertas: Oferta[], ok = true): Response {
  return { ok, json: vi.fn().mockResolvedValue(ofertas) } as unknown as Response;
}

function fabricar(): {
  pg: { query: Mock; queryOne: Mock };
  servicio: CarritoService;
  fetchMock: Mock;
} {
  const pg = {
    query: vi.fn().mockResolvedValue(undefined),
    queryOne: vi.fn().mockResolvedValue(undefined),
  };
  const fetchMock = vi.fn().mockResolvedValue(respuestaOfertas([]));
  vi.stubGlobal('fetch', fetchMock);
  return { pg, servicio: new CarritoService(pg as unknown as PgService), fetchMock };
}

afterEach(() => vi.unstubAllGlobals());

describe('CarritoService (RN-05, R-U-18, R-COV-2)', () => {
  it('devuelve una vista vacía cuando el comprador no tiene carrito', async () => {
    const { pg, servicio } = fabricar();

    await expect(servicio.obtener('comprador-1')).resolves.toEqual({
      items: [],
      total: '0.00',
      expira_en: null,
    });
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('DELETE FROM orders.carritos'), [
      CARRITO_EXPIRACION_MS,
    ]);
  });

  it('enriquece las líneas y calcula total y expiración con precios actuales', async () => {
    const { pg, servicio, fetchMock } = fabricar();
    pg.queryOne.mockResolvedValue({
      items_json: [item],
      total_cents: 1,
      actualizado_en: actualizado,
    });
    fetchMock.mockResolvedValue(respuestaOfertas([ofertaActiva]));

    const { id: _id, ...ofertaSinId } = ofertaActiva;
    await expect(servicio.obtener('comprador-1')).resolves.toEqual({
      items: [{ ...item, ...ofertaSinId }],
      total: '25.00',
      expira_en: new Date(new Date(actualizado).getTime() + CARRITO_EXPIRACION_MS).toISOString(),
    });
  });

  it('rechaza agregar una oferta inexistente o no activa', async () => {
    const { servicio, fetchMock } = fabricar();
    await expect(servicio.agregar('comprador-1', 'of-x', 1)).rejects.toBeInstanceOf(NotFoundError);

    fetchMock.mockResolvedValue(
      respuestaOfertas([{ ...ofertaActiva, estado: 'pausada' }]),
    );
    await expect(servicio.agregar('comprador-1', 'of-1', 1)).rejects.toMatchObject({
      codigo: 'OFERTA_NO_DISPONIBLE',
    });
  });

  it('impide superar la cantidad máxima por línea', async () => {
    const { pg, servicio, fetchMock } = fabricar();
    fetchMock.mockResolvedValue(respuestaOfertas([ofertaActiva]));
    pg.queryOne.mockResolvedValue({ items_json: [{ ...item, cantidad: 98 }] });

    await expect(
      servicio.agregar('comprador-1', 'of-1', 2),
    ).rejects.toMatchObject({ codigo: 'CANTIDAD_MAXIMA' });
    expect(CANTIDAD_MAXIMA_POR_LINEA).toBe(99);
  });

  it('suma una línea existente, persiste y devuelve la vista actualizada', async () => {
    const { pg, servicio, fetchMock } = fabricar();
    fetchMock
      .mockResolvedValueOnce(respuestaOfertas([ofertaActiva]))
      .mockResolvedValueOnce(respuestaOfertas([ofertaActiva]))
      .mockResolvedValueOnce(respuestaOfertas([ofertaActiva]));
    pg.queryOne
      .mockResolvedValueOnce({ items_json: [item] })
      .mockResolvedValueOnce({
        items_json: [{ oferta_id: 'of-1', cantidad: 5 }],
        actualizado_en: actualizado,
      });

    const vista = await servicio.agregar('comprador-1', 'of-1', 3);

    expect(vista.items[0].cantidad).toBe(5);
    expect(vista.total).toBe('62.50');
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('ON CONFLICT'), [
      'comprador-1',
      JSON.stringify([{ oferta_id: 'of-1', cantidad: 5 }]),
      6250,
    ]);
  });

  it('rechaza actualizar o quitar una línea ausente', async () => {
    const { servicio } = fabricar();

    await expect(servicio.actualizarCantidad('comprador-1', 'of-x', 2)).rejects.toBeInstanceOf(
      NotFoundError,
    );
    await expect(servicio.quitar('comprador-1', 'of-x')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('elimina la línea al actualizarla con cantidad cero', async () => {
    const { pg, servicio } = fabricar();
    pg.queryOne
      .mockResolvedValueOnce({ items_json: [item] })
      .mockResolvedValueOnce({ items_json: [item] })
      .mockResolvedValueOnce(undefined);

    await expect(servicio.actualizarCantidad('comprador-1', 'of-1', 0)).resolves.toEqual({
      items: [],
      total: '0.00',
      expira_en: null,
    });
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('ON CONFLICT'), [
      'comprador-1',
      '[]',
      0,
    ]);
  });

  it('vacía el carrito y expone las líneas para checkout', async () => {
    const { pg, servicio } = fabricar();
    pg.queryOne.mockResolvedValue({ items_json: [item] });

    await expect(servicio.vaciar('comprador-1')).resolves.toEqual({
      items: [],
      total: '0.00',
      expira_en: null,
    });
    await expect(servicio.itemsParaCheckout('comprador-1')).resolves.toEqual([item]);
    expect(pg.query).toHaveBeenCalledWith(
      'DELETE FROM orders.carritos WHERE comprador_id = $1',
      ['comprador-1'],
    );
  });

  it('vacía el carrito dentro de la transacción recibida', async () => {
    const { servicio } = fabricar();
    const client = { query: vi.fn().mockResolvedValue({ rowCount: 1 }) };

    await expect(servicio.vaciarEnTransaccion(client as never, 'comprador-1')).resolves.toEqual({
      rowCount: 1,
    });
    expect(client.query).toHaveBeenCalledWith(
      'DELETE FROM orders.carritos WHERE comprador_id = $1',
      ['comprador-1'],
    );
  });

  it('trata una oferta no disponible o un error de red como oferta no enriquecible', async () => {
    const { pg, servicio, fetchMock } = fabricar();
    pg.queryOne.mockResolvedValue({ items_json: [item], actualizado_en: actualizado });
    fetchMock.mockResolvedValue(respuestaOfertas([], false));

    const vista = await servicio.obtener('comprador-1');
    expect(vista.items).toEqual([]);
    expect(vista.expira_en).not.toBeNull();

    fetchMock.mockRejectedValue(new Error('stores offline'));
    await expect(servicio.obtener('comprador-1')).resolves.toMatchObject({ items: [] });
  });
});
