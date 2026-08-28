import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import { CreateOrderCommand } from './commands/create-order.command';
import { Money } from '@core/shared';

class PgFake {
  async transaccion<T>(fn: (client: unknown) => Promise<T>): Promise<T> {
    return fn({ query: jest.fn(async () => ({ rows: [] })) });
  }
}

class EventStoreFake {
  appendEnTransaccion = jest.fn(async (_c: unknown, orderId: string, tipo: string, datos: unknown) => ({
    secuencia: 1,
    order_id: orderId,
    tipo,
    datos,
    ocurrido_en: new Date().toISOString(),
  }));
  historiaDe = jest.fn();
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

  crearEnTransaccion = jest.fn(
    async (_c: unknown, vista: Omit<VistaFake, 'total' | 'actualizado_en'>) => {
      this.vistas.set(vista.id, {
        ...vista,
        total: Money.desdeCentavos(vista.total_cents).string(),
        actualizado_en: vista.creado_en,
      });
    },
  );
  actualizarEstado = jest.fn(async (id: string, estado: string, motivo?: string) => {
    const v = this.vistas.get(id);
    if (v) {
      v.estado = estado;
      v.motivo = motivo;
    }
  });
  encontrar = jest.fn(async (id: string) => this.vistas.get(id) ?? null);
  listarDeCliente = jest.fn();
  listarTodo = jest.fn();
  reconstruirDeHistoria = jest.fn();
}

class CarritoFake {
  itemsParaCheckout = jest.fn(async () => []);
  vaciarEnTransaccion = jest.fn();
}

/** Preparo el handler con fetch mockeado (stores + catalog internos). */
function crearHandler(
  sobreOfertas: unknown[],
  stockCatalog: 'ok' | 'insuficiente',
  carrito?: CarritoFake,
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
  global.fetch = jest.fn(
    async (url: string | URL | Request, _init?: RequestInit) => {
      const u = String(url);
      if (u.includes('stores-service')) {
        return { ok: true, json: async () => sobreOfertas } as Response;
      }
      if (u.includes('productos/lote')) {
        const skus = (u.match(/skus=([^&]*)/)?.[1] ?? '').split(',');
        const stock = stockCatalog === 'ok' ? 10 : 0;
        return {
          ok: true,
          json: async () => skus.map((sku) => ({ sku, stock, estado: stock > 0 ? 'disponible' : 'agotado' })),
        } as Response;
      }
      return { ok: false } as Response;
    },
  );
  return { handler, store, views };
}

describe('CreateOrderCommandHandler (CQRS + Event Sourcing)', () => {
  const ofertaActiva = {
    id: 'oferta-1',
    tienda_id: 'tienda-1',
    vendedor_id: 'vendedor-1',
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
});

describe('Calculo de comision TC-08 (RN-04)', () => {
  it('venta de C$ 1 150 con comision 12 %: comision 138.00, vendedor recibe 1 012.00', () => {
    const venta = Money.parsear('1150.00');
    expect(venta.comision(0.12).string()).toBe('138.00');
    expect(venta.restar(venta.comision(0.12)).string()).toBe('1012.00');
  });
});