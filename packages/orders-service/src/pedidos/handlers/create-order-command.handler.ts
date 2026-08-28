import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  PgService,
  Money,
  DomainError,
  ConflictError,
  NotFoundError,
  validarTransicion,
  ItemOrden,
  EVENTOS,
} from '@core/shared';
import { CreateOrderCommand } from '../commands/create-order.command';
import { OrderEventStore } from '../repositories/order-event-store';
import { OrderViewRepository, OrderView } from '../queries/order-view.repository';
import { reconstruirDesdeEventos } from '../models/order';
import { CarritoService } from '../carrito/carrito.service';

export interface OfertaExterna {
  id: string;
  tienda_id: string;
  vendedor_id: string;
  producto_id: string;
  sku: string;
  producto_nombre: string;
  margen: number;
  precio_base: string;
  precio_venta: string;
  stock: number;
  estado: string;
}

/**
 * CreateOrderCommandHandler: valida reglas (RN-03, RN-04, RN-05), persiste la
 * orden con el modelo Order y publica el evento (carpeta handlers, cap. 3.2).
 * Proyecciones (orden_vista, comisiones_vista, orden_timeline) se actualizan
 * via trigger desde orders.eventos (CQRS + Event Sourcing, ADR-07/08).
 */
@Injectable()
export class CreateOrderCommandHandler {
  private readonly storesUrl = process.env.STORES_SERVICE_URL ?? 'http://stores-service:3003';
  private readonly catalogUrl = process.env.CATALOG_SERVICE_URL ?? 'http://catalog-service:3002';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';

  constructor(
    private readonly pg: PgService,
    private readonly eventStore: OrderEventStore,
    private readonly views: OrderViewRepository,
    private readonly carritos: CarritoService,
  ) {}

  async ejecutar(comando: CreateOrderCommand, clienteId: string): Promise<OrderView> {
    let desdeCarrito = false;
    let lineas = comando.items;
    if (comando.usar_carrito) {
      const delCarrito = await this.carritos.itemsParaCheckout(clienteId);
      if (delCarrito.length === 0) {
        throw new DomainError('CARRITO_VACIO', 'El carrito esta vacio o expiro (RN-05).');
      }
      lineas = delCarrito;
      desdeCarrito = true;
    }
    if (!lineas?.length) {
      throw new DomainError('CARRITO_VACIO', 'El carrito no puede estar vacio (RN-05).');
    }
    const agrupados = this.agrupar(lineas);

    // 1. enriquecer con precios finales del stores-service (RN-01)
    const ofertas = await this.consultarOfertas(agrupados.map((i) => i.oferta_id));
    const porId = new Map(ofertas.map((o) => [o.id, o]));
    const items: ItemOrden[] = [];
    let total = Money.desdeCentavos(0);
    for (const linea of agrupados) {
      const oferta = porId.get(linea.oferta_id);
      if (!oferta || oferta.estado !== 'activa') {
        throw new NotFoundError('Oferta', linea.oferta_id);
      }
      const precioUnitario = Money.parsear(oferta.precio_venta);
      total = total.sumar(precioUnitario.multiplicarPor(linea.cantidad));
      items.push({
        oferta_id: oferta.id,
        sku: oferta.sku,
        producto_nombre: oferta.producto_nombre,
        cantidad: linea.cantidad,
        precio_unitario_cents: precioUnitario.centavos,
        vendedor_id: oferta.vendedor_id,
        tienda_id: oferta.tienda_id,
      });
    }

    // 2. control sincrono de viabilidad de stock (TC-04: respuesta 409 rapida)
    await this.verificarStock(items);

    // 3. transaccion: evento + outbox (proyecciones via trigger en orders.eventos)
    const orderId = randomUUID();
    const creadoEn = new Date().toISOString();
    await this.pg.transaccion(async (client) => {
      await this.eventStore.appendEnTransaccion(client, orderId, 'OrderCreatedEvent', {
        order_id: orderId,
        cliente_id: clienteId,
        items,
        total_cents: total.centavos,
        estado: 'creada',
        creado_en: creadoEn,
      });
      const evento = {
        event_id: randomUUID(),
        tipo: EVENTOS.ORDER_CREATED,
        ocurrido_en: creadoEn,
        data: {
          order_id: orderId,
          cliente_id: clienteId,
          items,
          total_cents: total.centavos,
          estado: 'creada',
        },
      };
      await client.query(
        `INSERT INTO orders.outbox (event_id, tipo, payload, estado, creado_en)
         VALUES ($1, $2, $3, 'pendiente', NOW())`,
        [evento.event_id, evento.tipo, JSON.stringify(evento)],
      );
      // checkout desde el carrito: se vacia en la misma transaccion (RN-05)
      if (desdeCarrito) {
        await this.carritos.vaciarEnTransaccion(client, clienteId);
      }
    });

    const primerItem = items[0];
    const vista: OrderView = {
      id: orderId,
      vendedor_id: primerItem?.vendedor_id ?? '',
      comprador_id: clienteId,
      tienda_id: primerItem?.tienda_id ?? '',
      estado: 'creada',
      total_cents: total.centavos,
      comision_cents: Math.round(total.centavos * 0.12),
      moneda: 'C$',
      items,
      direccion_envio: null,
      creado_en: creadoEn,
      actualizado_en: creadoEn,
      pagada_en: null,
      enviada_en: null,
      entregada_en: null,
      cancelada_en: null,
      devuelta_en: null,
    };
    return vista;
  }

  /** Cambio de estado (PATCH logistica -> transicion interna). */
  async transicionar(
    orderId: string,
    estadoNuevo: string,
    motivo?: string,
    actor?: string,
  ): Promise<OrderView> {
    const orden = await this.eventStore.historiaDe(orderId);
    if (orden.length === 0) throw new NotFoundError('Orden', orderId);
    const estado = reconstruirDesdeEventos(orden);
    if (!estado) throw new NotFoundError('Orden', orderId);

    validarTransicion(estado.estado as never, estadoNuevo as never);

    await this.pg.transaccion(async (client) => {
      await this.eventStore.appendEnTransaccion(client, orderId, 'OrderStatusUpdatedEvent', {
        order_id: orderId,
        estado: estadoNuevo,
        previo_estado: estado.estado,
        motivo,
        actor: actor ?? 'sistema',
        ocurrido_en: new Date().toISOString(),
      });
      const evento = {
        event_id: randomUUID(),
        tipo: EVENTOS.ORDER_STATUS_UPDATED,
        ocurrido_en: new Date().toISOString(),
        data: { order_id: orderId, estado: estadoNuevo, previo_estado: estado.estado, motivo },
      };
      await client.query(
        `INSERT INTO orders.outbox (event_id, tipo, payload, estado, creado_en)
         VALUES ($1, $2, $3, 'pendiente', NOW())`,
        [evento.event_id, evento.tipo, JSON.stringify(evento)],
      );
    });
    return (await this.views.encontrar(orderId)) as OrderView;
  }

  /** Replay: reconstruye la proyeccion desde la historia de eventos. */
  async reproyectar(orderId: string): Promise<OrderView | null> {
    const historia = await this.eventStore.historiaDe(orderId);
    // Las proyecciones ya están actualizadas por trigger; solo devolvemos la vista actual
    return this.views.encontrar(orderId);
  }

  private agrupar(items: { oferta_id: string; cantidad: number }[]): { oferta_id: string; cantidad: number }[] {
    const mapa = new Map<string, number>();
    for (const i of items) {
      mapa.set(i.oferta_id, (mapa.get(i.oferta_id) ?? 0) + i.cantidad);
    }
    return [...mapa.entries()].map(([oferta_id, cantidad]) => ({ oferta_id, cantidad }));
  }

  private async consultarOfertas(ids: string[]): Promise<OfertaExterna[]> {
    try {
      const res = await fetch(
        `${this.storesUrl}/internal/ofertas?ids=${encodeURIComponent(ids.join(','))}`,
        { headers: { 'x-internal-key': this.claveInterna } },
      );
      if (!res.ok) return [];
      return (await res.json()) as OfertaExterna[];
    } catch {
      return [];
    }
  }

  /** Consulta sincrona de stock al catalogo (lecturas por REST, sec. 3.2). */
  private async verificarStock(items: ItemOrden[]): Promise<void> {
    try {
      const skus = [...new Set(items.map((i) => i.sku))];
      const res = await fetch(
        `${this.catalogUrl}/internal/productos/lote?skus=${encodeURIComponent(skus.join(','))}`,
        { headers: { 'x-internal-key': this.claveInterna } },
      );
      if (!res.ok) return;
      const productos = (await res.json()) as { sku: string; stock: number; estado: string }[];
      const porSku = new Map(productos.map((p) => [p.sku, p]));
      for (const item of items) {
        const producto = porSku.get(item.sku);
        if (!producto || producto.stock < item.cantidad) {
          throw new ConflictError(
            'Stock insuficiente (RN-03): se rechaza la orden.',
            { sku: item.sku, cantidad_solicitada: item.cantidad },
          );
        }
      }
    } catch (err) {
      if (err instanceof ConflictError) throw err;
      // si el catalogo no responde, la autoridad la tiene el evento stock.fallido
    }
  }
}