import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  PgService,
  Money,
  DomainError,
  ConflictError,
  CapabilityDisabledError,
  FULFILLMENTS,
  NotFoundError,
  Fulfillment,
  TIPOS_ORDEN,
  TipoOrden,
  validarTransicionPorTipo,
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
  empresa_id: string;
  participacion_porcentaje: number;
  item_type: 'producto' | 'servicio' | 'alquiler';
  item_id: string;
  sku: string;
  producto_nombre: string;
  margen: number;
  precio_base: string;
  precio_venta: string;
  stock: number;
  estado: string;
}

interface LineaAgrupada {
  oferta_id: string;
  cantidad: number;
  fulfillment?: Fulfillment;
  fulfillment_config?: Record<string, unknown>;
}

/**
 * CreateOrderCommandHandler: valida reglas (RN-03, RN-04, RN-05), persiste la
 * orden con el modelo Order y publica el evento (carpeta handlers, cap. 3.2).
 * Proyecciones (orden_vista, orden_timeline) se actualizan
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
    const tipoOrden = this.validarTipoOrden(comando.tipo_orden);
    let desdeCarrito = false;
    let lineas = comando.items;
    if (comando.usar_carrito) {
      const delCarrito = await this.carritos.itemsParaCheckout(clienteId);
      if (delCarrito.length === 0) {
        throw new DomainError('CARRITO_VACIO', 'El carrito esta vacio o expiro (RN-05).');
      }
      lineas = delCarrito.map((linea) => ({ ...linea, fulfillment: 'none' as Fulfillment }));
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
      const itemType = oferta.item_type;
      const fulfillment = this.validarFulfillment(linea.fulfillment, tipoOrden, itemType);
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
        empresa_id: oferta.empresa_id,
        participacion_porcentaje: oferta.participacion_porcentaje,
        item_type: itemType,
        item_id: oferta.item_id,
        fulfillment,
        fulfillment_config: linea.fulfillment_config,
      });
    }

    await this.validarCapacidades(items, tipoOrden);

    // 2. control sincrono de viabilidad de stock (TC-04: respuesta 409 rapida)
    await this.verificarStock(items);

    // 3. transaccion: evento + outbox (proyecciones via trigger en orders.eventos)
    const orderId = randomUUID();
    const creadoEn = new Date().toISOString();
    await this.pg.transaccion(async (client) => {
      await this.eventStore.appendEnTransaccion(client, orderId, 'OrderCreatedEvent', {
        order_id: orderId,
        cliente_id: clienteId,
        ...(comando.cliente_parte_id ? { cliente_parte_id: comando.cliente_parte_id } : {}),
        items,
        total_cents: total.centavos,
        estado: 'creada',
        tipo_orden: tipoOrden,
        fulfillment_config: this.resumenFulfillment(items),
        creado_en: creadoEn,
      });
      const evento = {
        event_id: randomUUID(),
        tipo: EVENTOS.ORDER_CREATED,
        ocurrido_en: creadoEn,
        data: {
          order_id: orderId,
          cliente_id: clienteId,
          ...(comando.cliente_parte_id ? { cliente_parte_id: comando.cliente_parte_id } : {}),
          items,
          total_cents: total.centavos,
          estado: 'creada',
          tipo_orden: tipoOrden,
          fulfillment_config: this.resumenFulfillment(items),
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
      comision_cents: total.comision(0.12).centavos,
      moneda: 'C$',
      items,
      direccion_envio: null,
      creado_en: creadoEn,
      actualizado_en: creadoEn,
      tipo_orden: tipoOrden,
      fulfillment_config: this.resumenFulfillment(items),
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

    validarTransicionPorTipo(
      (estado.tipo_orden ?? 'compra') as TipoOrden,
      estado.estado as never,
      estadoNuevo as never,
    );
    const actorFinal = actor ?? 'sistema';

    await this.pg.transaccion(async (client) => {
      await this.eventStore.appendEnTransaccion(client, orderId, 'OrderStatusUpdatedEvent', {
        order_id: orderId,
        estado: estadoNuevo,
        previo_estado: estado.estado,
        motivo,
        actor: actorFinal,
        ocurrido_en: new Date().toISOString(),
      });
      const evento = {
        event_id: randomUUID(),
        tipo: EVENTOS.ORDER_STATUS_UPDATED,
        ocurrido_en: new Date().toISOString(),
        data: {
          order_id: orderId,
          estado: estadoNuevo,
          previo_estado: estado.estado,
          motivo,
          actor: actorFinal,
        },
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
    // Las proyecciones ya están actualizadas por trigger; solo devolvemos la vista actual
    return this.views.encontrar(orderId);
  }

  private agrupar(items: { oferta_id: string; cantidad: number; fulfillment?: Fulfillment; fulfillment_config?: Record<string, unknown> }[]): LineaAgrupada[] {
    const mapa = new Map<string, LineaAgrupada>();
    for (const i of items) {
      const actual = mapa.get(i.oferta_id);
      if (actual && actual.fulfillment !== i.fulfillment) {
        throw new DomainError('FULFILLMENT_CONFLICTO', 'Una misma oferta no puede tener dos fulfillments distintos en una orden.');
      }
      mapa.set(i.oferta_id, {
        oferta_id: i.oferta_id,
        cantidad: (actual?.cantidad ?? 0) + i.cantidad,
        fulfillment: i.fulfillment ?? actual?.fulfillment,
        fulfillment_config: i.fulfillment_config ?? actual?.fulfillment_config,
      });
    }
    return [...mapa.values()];
  }

  private validarTipoOrden(valor: string): TipoOrden {
    if (!valor) {
      throw new DomainError('TIPO_ORDEN_REQUERIDO', 'El tipo de orden es obligatorio.');
    }
    if (!TIPOS_ORDEN.includes(valor as TipoOrden)) {
      throw new DomainError('TIPO_ORDEN_INVALIDO', 'El tipo de orden no es valido.');
    }
    return valor as TipoOrden;
  }

  private validarFulfillment(
    valor: Fulfillment | undefined,
    tipoOrden: TipoOrden,
    itemType: 'producto' | 'servicio' | 'alquiler',
  ): Fulfillment {
    const fulfillment = valor ?? 'none';
    if (!FULFILLMENTS.includes(fulfillment as Fulfillment)) {
      throw new DomainError('FULFILLMENT_INVALIDO', 'El fulfillment no es valido.');
    }
    if (fulfillment === 'delivery' && tipoOrden === 'reserva') {
      throw new DomainError('FULFILLMENT_INVALIDO', 'Una reserva no puede usar delivery.');
    }
    if (tipoOrden === 'reserva' && itemType !== 'servicio') {
      throw new DomainError('TIPO_ITEM_INVALIDO', 'Las reservas solo aplican a servicios.');
    }
    return fulfillment;
  }

  private resumenFulfillment(items: ItemOrden[]): Record<string, unknown> {
    return {
      por_linea: items.map((item) => ({
        oferta_id: item.oferta_id,
        fulfillment: item.fulfillment ?? 'none',
        config: item.fulfillment_config ?? {},
      })),
    };
  }

  private async validarCapacidades(items: ItemOrden[], tipoOrden: TipoOrden): Promise<void> {
    const requeridas = new Set<string>();
    if (tipoOrden === 'cotizacion') requeridas.add('cotizaciones');
    for (const item of items) {
      if (item.fulfillment === 'delivery') requeridas.add('fulfillment_delivery');
      if (item.fulfillment === 'pickup') requeridas.add('fulfillment_pickup');
      if (item.fulfillment === 'onsite') requeridas.add('fulfillment_onsite');
    }
    if (requeridas.size === 0) return;
    const empresas = [...new Set(items.map((item) => item.empresa_id))];
    const filas = await this.pg.query<{
      empresa_id: string;
      capacidad_codigo: string;
      activo: boolean;
    }>(
      `SELECT empresa_id, capacidad_codigo, activo
         FROM identity.empresa_capacidades
        WHERE empresa_id = ANY($1::uuid[])
          AND capacidad_codigo = ANY($2::text[])`,
      [empresas, [...requeridas]],
    );
    const activas = new Set(
      (Array.isArray(filas) ? filas : [])
        .filter((fila) => fila.activo)
        .map((fila) => `${fila.empresa_id}:${fila.capacidad_codigo}`),
    );
    for (const empresa of empresas) {
      const faltantes = [...requeridas].filter((capacidad) => !activas.has(`${empresa}:${capacidad}`));
      if (faltantes.length > 0) throw new CapabilityDisabledError(faltantes, empresa);
    }
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
      const productosConStock = items.filter((item) => item.item_type === 'producto');
      if (productosConStock.length === 0) return;
      const skus = [...new Set(productosConStock.map((i) => i.sku))];
      const res = await fetch(
        `${this.catalogUrl}/internal/productos/lote?skus=${encodeURIComponent(skus.join(','))}`,
        { headers: { 'x-internal-key': this.claveInterna } },
      );
      if (!res.ok) return;
      const productos = (await res.json()) as { empresa_id: string; sku: string; stock: number; estado: string }[];
      // ADR-27: el SKU se repite entre empresas; la clave es (empresa, SKU).
      const porClave = new Map(
        productos.map((p) => [`${p.empresa_id}:${p.sku.toLowerCase()}`, p]),
      );
      for (const item of productosConStock) {
        const producto = porClave.get(
          `${item.empresa_id}:${item.sku.toLowerCase()}`,
        );
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
