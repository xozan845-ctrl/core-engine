import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import { CreateOrderCommand } from './commands/create-order.command';
import { Money } from '@core/shared';

class PgFake {
  query = vi.fn(async () => ({ rows: [] }));
  async transaccion<T>(fn: (client: unknown) => Promise<T>): Promise<T> {
    return fn({ query: this.query });
  }
}

class EventStoreFake {
  appendEnTransaccion = vi.fn(async (_c: unknown, orderId: string, tipo: string, datos: unknown) => ({
    secuencia: 1,
    order_id: orderId,
    tipo,
    datos,
    ocurrido_en: new Date().toISOString(),
  }));
  historiaDe = vi.fn();
}

interface VistaFake {
  id: string;
  cliente_id: string;
  items: unknown[];
  total: string;
  total_cents: number;
  estado: string;
  motivo?: string;
  creado_en: string;
  actualizado_en: string;
}

class ViewsFake {
  private vistas = new Map<string, VistaFake>();

  crearEnTransaccion = vi.fn(
    async (_c: unknown, vista: Omit<VistaFake, 'total' | 'actualizado_en'>) => {
      this.vistas.set(vista.id, {
        ...vista,
        total: Money.desdeCentavos(vista.total_cents).string(),
        actualizado_en: vista.creado_en,
      });
    },
  );
  actualizarEstado = vi.fn(async (id: string, estado: string, motivo?: string) => {
    const v = this.vistas.get(id);
    if (v) {
      v.estado = estado;
      v.motivo = motivo;
    }
  });
  encontrar = vi.fn(async (id: string) => this.vistas.get(id) ?? null);
  listarDeCliente = vi.fn();
  listarTodo = vi.fn();
  reconstruirDeHistoria = vi.fn();
}

class CarritoFake {
  itemsParaCheckout = vi.fn(async () => []);
  vaciarEnTransaccion = vi.fn();
}

type StockMode =
  | 'ok'
  | 'insuficiente'
  | 'sin-sku'
  | 'fetch-error'
  | 'network-error';

/** Preparo el handler con fetch mockeado (stores + catalog internos). */
function crearHandler(
  sobreOfertas: unknown[],
  stockCatalog: StockMode,
  carrito?: CarritoFake,
  storesFails = false,
  storesThrows = false,
) {
  const pg = new PgFake();
  const store = new EventStoreFake();
  const views = new ViewsFake();
  const handler = new CreateOrderCommandHandler(
    pg as never,
    store as never,
    views as never,
    (carrito ?? new CarritoFake()) as never,
  );
  const fetchMock = vi.fn(
    async (url: string | URL | Request, _init?: RequestInit) => {
      const u = String(url);
      if (u.includes('stores-service')) {
        if (storesThrows) throw new Error('stores down');
        if (storesFails) return { ok: false } as Response;
        return { ok: true, json: async () => sobreOfertas } as Response;
      }
      if (u.includes('productos/lote')) {
        if (stockCatalog === 'fetch-error') return { ok: false } as Response;
        if (stockCatalog === 'network-error') throw new Error('network down');
        if (stockCatalog === 'sin-sku') {
          return { ok: true, json: async () => [] } as Response;
        }
        const skus = decodeURIComponent(
          u.match(/skus=([^&]*)/)?.[1] ?? '',
        ).split(',');
        const stock = stockCatalog === 'ok' ? 10 : 0;
        return {
          ok: true,
          json: async () =>
            skus.map((sku) => ({
              sku,
              stock,
              estado: stock > 0 ? 'disponible' : 'agotado',
            })),
        } as Response;
      }
      return { ok: false } as Response;
    },
  );
  global.fetch = fetchMock;
  return { handler, store, views, fetchMock, carrito, pg };
}

describe('CreateOrderCommandHandler (CQRS + Event Sourcing)', () => {
  const ofertaActiva = {
    id: 'oferta-1',
    tienda_id: 'tienda-1',
    vendedor_id: 'vendedor-1',
    empresa_id: 'empresa-1',
    participacion_porcentaje: 25,
    producto_id: 'prod-1',
    sku: 'ACE-001',
    producto_nombre: 'Aceite 1L',
    margen: 15,
    precio_base: '1000.00',
    precio_venta: '1150.00',
    stock: 10,
    estado: 'activa',
  };

  it('TC-02/TC-03: calcula el total con las reglas RN-01/RN-04 y persiste evento OrderCreatedEvent', async () => {
    const { handler, store } = crearHandler([ofertaActiva], 'ok');
    const comando: CreateOrderCommand = {
      items: [{ oferta_id: 'oferta-1', cantidad: 1 }],
    };
    await handler.ejecutar(comando, 'comprador-1');

    const appendCall = store.appendEnTransaccion.mock.calls[0];
    expect(appendCall[2]).toBe('OrderCreatedEvent');
    const datos = appendCall[3] as { total_cents: number; items: { cantidad: number }[] };
    expect(datos.total_cents).toBe(115000); // 1 x C$ 1 150 (base 1000 x 1,15)
    expect(datos.items).toHaveLength(1);
    expect(datos.items[0]).toMatchObject({
      empresa_id: 'empresa-1',
      tienda_id: 'tienda-1',
      vendedor_id: 'vendedor-1',
      participacion_porcentaje: 25,
    });
  });

  it('total con cantidades multiples: 2 unidades = C$ 2 300', async () => {
    const { handler } = crearHandler([ofertaActiva], 'ok');
    const comando: CreateOrderCommand = {
      items: [{ oferta_id: 'oferta-1', cantidad: 2 }],
    };
    const vista = await handler.ejecutar(comando, 'comprador-1');
    expect(vista.total_cents).toBe(230000);
  });

  it('agrupa lineas repetidas del carrito (RN-05: cantidades sumadas)', async () => {
    const { handler } = crearHandler([ofertaActiva], 'ok');
    const comando: CreateOrderCommand = {
      items: [
        { oferta_id: 'oferta-1', cantidad: 1 },
        { oferta_id: 'oferta-1', cantidad: 2 },
      ],
    };
    const vista = await handler.ejecutar(comando, 'comprador-1');
    expect(vista.total_cents).toBe(345000); // 3 x 1150.00
  });

  it('carrito vacio se rechaza (RN-05)', async () => {
    const { handler } = crearHandler([ofertaActiva], 'ok');
    await expect(handler.ejecutar({ items: [] }, 'comprador-1')).rejects.toMatchObject({
      codigo: 'CARRITO_VACIO',
    });
  });

  it('oferta inexistente -> 404', async () => {
    const { handler } = crearHandler([], 'ok');
    await expect(
      handler.ejecutar({ items: [{ oferta_id: 'oferta-x', cantidad: 1 }] }, 'comprador-1'),
    ).rejects.toMatchObject({ codigo: 'NO_ENCONTRADO' });
  });

  it('TC-04: viabilidad de stock rechaza con 409/stock_insuficiente como CONFLICTO', async () => {
    const { handler } = crearHandler([ofertaActiva], 'insuficiente');
    await expect(
      handler.ejecutar({ items: [{ oferta_id: 'oferta-1', cantidad: 1 }] }, 'comprador-1'),
    ).rejects.toMatchObject({ codigo: 'CONFLICTO' });
  });

  it('persiste el payload completo del evento, la vista y el outbox', async () => {
    const carrito = new CarritoFake();
    const { handler, store, pg, fetchMock } = crearHandler(
      [ofertaActiva],
      'ok',
      carrito,
    );
    const vista = await handler.ejecutar(
      { items: [{ oferta_id: 'oferta-1', cantidad: 2 }] },
      'comprador-1',
    );

    const append = store.appendEnTransaccion.mock.calls[0];
    const orderId = append[1] as string;
    const datos = append[3] as Record<string, unknown>;
    expect(append[2]).toBe('OrderCreatedEvent');
    expect(datos).toMatchObject({
      order_id: orderId,
      cliente_id: 'comprador-1',
      total_cents: 230_000,
      estado: 'creada',
      items: [
        {
          oferta_id: 'oferta-1',
          sku: 'ACE-001',
          producto_nombre: 'Aceite 1L',
          cantidad: 2,
          precio_unitario_cents: 115_000,
          vendedor_id: 'vendedor-1',
          tienda_id: 'tienda-1',
        },
      ],
    });
    expect(typeof datos.creado_en).toBe('string');

    expect(vista).toMatchObject({
      id: orderId,
      vendedor_id: 'vendedor-1',
      comprador_id: 'comprador-1',
      tienda_id: 'tienda-1',
      estado: 'creada',
      total_cents: 230_000,
      comision_cents: 27_600,
      moneda: 'C$',
      direccion_envio: null,
      pagada_en: null,
      enviada_en: null,
      entregada_en: null,
      cancelada_en: null,
      devuelta_en: null,
    });
    expect(vista.creado_en).toBe(vista.actualizado_en);
    expect(carrito.vaciarEnTransaccion).not.toHaveBeenCalled();

    expect(fetchMock.mock.calls[0][0]).toBe(
      'http://stores-service:3003/internal/ofertas?ids=oferta-1',
    );
    expect(fetchMock.mock.calls[0][1]).toMatchObject({
      headers: { 'x-internal-key': '' },
    });
    expect(fetchMock.mock.calls[1][0]).toBe(
      'http://catalog-service:3002/internal/productos/lote?skus=ACE-001',
    );
    expect(fetchMock.mock.calls[1][1]).toMatchObject({
      headers: { 'x-internal-key': '' },
    });
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.outbox'),
      [
        expect.any(String),
        expect.any(String),
        expect.stringMatching(/"order_id"[\s\S]*"estado":"creada"/),
      ],
    );
  });

  it('checkout desde carrito usa sus líneas y lo vacía en la transacción', async () => {
    const carrito = new CarritoFake();
    carrito.itemsParaCheckout.mockResolvedValue([
      { oferta_id: 'oferta-1', cantidad: 1 },
    ]);
    const { handler } = crearHandler([ofertaActiva], 'ok', carrito);

    const vista = await handler.ejecutar({ usar_carrito: true }, 'comprador-1');

    expect(carrito.itemsParaCheckout).toHaveBeenCalledWith('comprador-1');
    expect(carrito.vaciarEnTransaccion).toHaveBeenCalledTimes(1);
    expect(vista.total_cents).toBe(115_000);
  });

  it('carrito vacío al usar_carrito -> CARRITO_VACIO', async () => {
    const carrito = new CarritoFake();
    carrito.itemsParaCheckout.mockResolvedValue([]);
    const { handler } = crearHandler([ofertaActiva], 'ok', carrito);

    await expect(
      handler.ejecutar({ usar_carrito: true }, 'comprador-1'),
    ).rejects.toMatchObject({
      codigo: 'CARRITO_VACIO',
      message: expect.stringContaining('carrito esta vacio'),
    });
  });

  it('items ausente -> CARRITO_VACIO', async () => {
    const { handler } = crearHandler([ofertaActiva], 'ok');
    await expect(handler.ejecutar({}, 'comprador-1')).rejects.toMatchObject({
      codigo: 'CARRITO_VACIO',
      message: expect.stringContaining('no puede estar vacio'),
    });
  });

  it('oferta inactiva -> NOT_FOUND con la entidad Oferta', async () => {
    const { handler } = crearHandler(
      [{ ...ofertaActiva, estado: 'inactiva' }],
      'ok',
    );
    await expect(
      handler.ejecutar({ items: [{ oferta_id: 'oferta-1', cantidad: 1 }] }, 'comprador-1'),
    ).rejects.toMatchObject({
      codigo: 'NO_ENCONTRADO',
      message: expect.stringContaining('Oferta oferta-1'),
    });
  });

  it('si stores-service responde !ok, la oferta no existe -> NOT_FOUND', async () => {
    const { handler } = crearHandler([], 'ok', undefined, true);
    await expect(
      handler.ejecutar({ items: [{ oferta_id: 'oferta-1', cantidad: 1 }] }, 'comprador-1'),
    ).rejects.toMatchObject({
      codigo: 'NO_ENCONTRADO',
      message: expect.stringContaining('Oferta oferta-1'),
    });
  });

  it('si stores-service lanza, la oferta no existe -> NOT_FOUND', async () => {
    const { handler } = crearHandler([], 'ok', undefined, false, true);
    await expect(
      handler.ejecutar({ items: [{ oferta_id: 'oferta-1', cantidad: 1 }] }, 'comprador-1'),
    ).rejects.toMatchObject({
      codigo: 'NO_ENCONTRADO',
      message: expect.stringContaining('Oferta oferta-1'),
    });
  });

  it('si el catálogo responde !ok, la orden se crea sin bloquear por stock', async () => {
    const { handler } = crearHandler([ofertaActiva], 'fetch-error');
    const vista = await handler.ejecutar(
      { items: [{ oferta_id: 'oferta-1', cantidad: 1 }] },
      'comprador-1',
    );
    expect(vista.total_cents).toBe(115_000);
  });

  it('si el catálogo lanza, la orden se crea (autoridad del evento stock.fallido)', async () => {
    const { handler } = crearHandler([ofertaActiva], 'network-error');
    const vista = await handler.ejecutar(
      { items: [{ oferta_id: 'oferta-1', cantidad: 1 }] },
      'comprador-1',
    );
    expect(vista.total_cents).toBe(115_000);
  });

  it('si el catálogo no devuelve el sku -> CONFLICTO con detalles', async () => {
    const { handler } = crearHandler([ofertaActiva], 'sin-sku');
    const error = await handler
      .ejecutar({ items: [{ oferta_id: 'oferta-1', cantidad: 1 }] }, 'comprador-1')
      .catch((e) => e);

    expect(error.codigo).toBe('CONFLICTO');
    expect(error.message).toContain('Stock insuficiente');
    expect(error.detalles).toEqual({ sku: 'ACE-001', cantidad_solicitada: 1 });
  });

  it('stock igual a la cantidad es válido (límite estricto)', async () => {
    const { handler } = crearHandler([ofertaActiva], 'ok');
    const vista = await handler.ejecutar(
      { items: [{ oferta_id: 'oferta-1', cantidad: 10 }] },
      'comprador-1',
    );
    expect(vista.total_cents).toBe(1_150_000);
  });

  it('agrupa ofertas distintas y suma sus totales', async () => {
    const segunda = {
      ...ofertaActiva,
      id: 'oferta-2',
      sku: 'ACE-002',
      producto_nombre: 'Aceite 2L',
      precio_venta: '2300.00',
    };
    const { handler } = crearHandler([ofertaActiva, segunda], 'ok');

    const vista = await handler.ejecutar(
      {
        items: [
          { oferta_id: 'oferta-1', cantidad: 1 },
          { oferta_id: 'oferta-2', cantidad: 2 },
        ],
      },
      'comprador-1',
    );
    expect(vista.total_cents).toBe(115_000 + 460_000);
    expect(vista.items).toHaveLength(2);
  });

  it('transicionar sin historia -> NOT_FOUND', async () => {
    const { handler, store } = crearHandler([ofertaActiva], 'ok');
    store.historiaDe.mockResolvedValue([]);
    await expect(handler.transicionar('orden-1', 'pagada')).rejects.toMatchObject({
      codigo: 'NO_ENCONTRADO',
      message: expect.stringContaining('Orden orden-1'),
    });
  });

  it('transicionar con historia sin OrderCreatedEvent -> NOT_FOUND', async () => {
    const { handler, store } = crearHandler([ofertaActiva], 'ok');
    store.historiaDe.mockResolvedValue([
      { tipo: 'OrderStatusUpdatedEvent', payload: {}, creado_en: '2026-01-01' },
    ]);
    await expect(handler.transicionar('orden-1', 'pagada')).rejects.toMatchObject({
      codigo: 'NO_ENCONTRADO',
      message: expect.stringContaining('Orden orden-1'),
    });
  });

  it('transicionar inválida -> TRANSICION_INVALIDA', async () => {
    const { handler, store } = crearHandler([ofertaActiva], 'ok');
    store.historiaDe.mockResolvedValue([
      {
        tipo: 'OrderCreatedEvent',
        payload: {
          order_id: 'orden-1',
          cliente_id: 'comprador-1',
          items: [],
          total_cents: 100,
          estado: 'creada',
          creado_en: '2026-01-01T00:00:00.000Z',
        },
        creado_en: '2026-01-01T00:00:00.000Z',
      },
    ]);
    await expect(
      handler.transicionar('orden-1', 'entregada'),
    ).rejects.toMatchObject({ codigo: 'TRANSICION_INVALIDA' });
  });

  it('transicionar válida persiste actor, motivo y outbox', async () => {
    const { handler, store, views, pg } = crearHandler([ofertaActiva], 'ok');
    const vista = {
      id: 'orden-1',
      vendedor_id: 'vendedor-1',
      comprador_id: 'comprador-1',
      tienda_id: 'tienda-1',
      estado: 'pagada',
      total_cents: 100,
      comision_cents: 12,
      moneda: 'C$',
      items: [],
      direccion_envio: null,
      creado_en: '2026-01-01T00:00:00.000Z',
      actualizado_en: '2026-01-01T00:00:00.000Z',
      pagada_en: null,
      enviada_en: null,
      entregada_en: null,
      cancelada_en: null,
      devuelta_en: null,
    };
    store.historiaDe.mockResolvedValue([
      {
        tipo: 'OrderCreatedEvent',
        payload: {
          order_id: 'orden-1',
          cliente_id: 'comprador-1',
          items: [],
          total_cents: 100,
          estado: 'creada',
          creado_en: '2026-01-01T00:00:00.000Z',
        },
        creado_en: '2026-01-01T00:00:00.000Z',
      },
    ]);
    views.encontrar.mockResolvedValue(vista);

    const resultado = await handler.transicionar(
      'orden-1',
      'pagada',
      'pago confirmado',
      'logistica',
    );

    const append = store.appendEnTransaccion.mock.calls[0];
    expect(append[2]).toBe('OrderStatusUpdatedEvent');
    expect(append[3]).toMatchObject({
      order_id: 'orden-1',
      estado: 'pagada',
      previo_estado: 'creada',
      motivo: 'pago confirmado',
      actor: 'logistica',
    });
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO orders.outbox'),
      [expect.any(String), expect.any(String), expect.stringContaining('"pagada"')],
    );
    expect(resultado).toBe(vista);
  });

  it('transicionar sin actor usa sistema', async () => {
    const { handler, store, views } = crearHandler([ofertaActiva], 'ok');
    store.historiaDe.mockResolvedValue([
      {
        tipo: 'OrderCreatedEvent',
        payload: {
          order_id: 'orden-1',
          cliente_id: 'comprador-1',
          items: [],
          total_cents: 100,
          estado: 'creada',
          creado_en: '2026-01-01T00:00:00.000Z',
        },
        creado_en: '2026-01-01T00:00:00.000Z',
      },
    ]);
    views.encontrar.mockResolvedValue(null);

    await handler.transicionar('orden-1', 'cancelada');
    expect(store.appendEnTransaccion.mock.calls[0][3]).toMatchObject({
      actor: 'sistema',
    });
  });

  it('reproyectar devuelve la vista actual', async () => {
    const { handler, views } = crearHandler([ofertaActiva], 'ok');
    const vista = { id: 'orden-1' };
    views.encontrar.mockResolvedValue(vista);

    await expect(handler.reproyectar('orden-1')).resolves.toBe(vista);
  });
});

describe('Calculo de comision TC-08 (RN-04)', () => {
  it('venta de C$ 1 150 con comision 12 %: comision 138.00, vendedor recibe 1 012.00', () => {
    const venta = Money.parsear('1150.00');
    expect(venta.comision(0.12).string()).toBe('138.00');
    expect(venta.restar(venta.comision(0.12)).string()).toBe('1012.00');
  });
});
