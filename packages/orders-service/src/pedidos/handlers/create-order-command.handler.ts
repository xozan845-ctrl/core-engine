import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  PgService,
  Money,
  DomainError,
  ConflictError,
  CapabilityDisabledError,
  ServiceUnavailableError,
  capacidadesDeEmpresas,
  FULFILLMENTS,
  NotFoundError,
  Fulfillment,
  TIPOS_ORDEN,
  TipoOrden,
  validarTransicionPorTipo,
  ItemOrden,
  EVENTOS,
  timeoutInternoMs,
  registrarCreditoOrden,
  anularCreditoOrden,
  cotizarItemReserva,
  verificarDisponibilidadReserva,
  resolverPreciosItems,
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
  /** RN-04: misma tasa que commissions-service y el trigger de proyección. */
  private readonly tasaComision = Number(process.env.COMMISSION_RATE ?? 0.12);

  constructor(
    private readonly pg: PgService,
    private readonly eventStore: OrderEventStore,
    private readonly views: OrderViewRepository,
    private readonly carritos: CarritoService,
  ) {}

  async ejecutar(comando: CreateOrderCommand, clienteId: string): Promise<OrderView> {
    const tipoOrden = this.validarTipoOrden(comando.tipo_orden);
    // Una cotizacion es una propuesta de precio: no reserva stock ni genera
    // comision hasta que el cliente la acepte y se convierta en orden.
    const esCotizacion = tipoOrden === 'cotizacion';
    const esReserva = tipoOrden === 'reserva';
    const metodoPago: 'simulado' | 'credito' =
      comando.metodo_pago === 'credito' ? 'credito' : 'simulado';
    if (esReserva && !comando.cliente_parte_id) {
      throw new DomainError('PARTE_REQUERIDA', 'Una reserva requiere cliente_parte_id.');
    }
    if (metodoPago === 'credito' && !comando.cliente_parte_id) {
      throw new DomainError(
        'CREDITO_REQUIERE_PARTE',
        'La venta a credito requiere identificar al cliente (cliente_parte_id).',
      );
    }
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
    const preciosPorOferta = await this.resolverPrecios(agrupados, porId, comando);
    const items: ItemOrden[] = [];
    let total = Money.desdeCentavos(0);
    for (const linea of agrupados) {
      const oferta = porId.get(linea.oferta_id);
      if (!oferta || oferta.estado !== 'activa') {
        throw new NotFoundError('Oferta', linea.oferta_id);
      }
      const itemType = oferta.item_type;
      const fulfillment = this.validarFulfillment(linea.fulfillment, tipoOrden, itemType);
      let precioUnitario = Money.parsear(oferta.precio_venta);
      const resuelto = preciosPorOferta.get(oferta.id);
      // La lista o escala sustituye el precio base; el margen del vendedor se
      // mantiene (RN-01). Sin lista/escala aplicable se conserva el snapshot
      // de la oferta para no alterar ventas existentes.
      if (resuelto && resuelto.origen !== 'base') {
        precioUnitario = Money.desdeCentavos(resuelto.precio_unitario_cents)
          .aplicarMargen(oferta.margen);
      }
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

    if (metodoPago === 'credito') {
      const empresas = new Set(items.map((item) => item.empresa_id));
      if (empresas.size > 1) {
        throw new DomainError(
          'CREDITO_UNA_EMPRESA',
          'Una venta a credito solo puede incluir items de una empresa.',
        );
      }
    }

    await this.validarCapacidades(items, tipoOrden, metodoPago);

    // Reservas: el precio se calcula por rango (tarifas) y se valida que el
    // recurso esté libre. Se hace después de las capacidades para fallar
    // cerrado antes de llamar a otros servicios.
    if (esReserva) {
      total = Money.desdeCentavos(0);
      for (let indice = 0; indice < items.length; indice += 1) {
        const item = items[indice];
        const linea = agrupados[indice];
        const config = linea.fulfillment_config ?? {};
        const recursoId = this.textoConfig(config, 'recurso_id');
        const iniciaEn = this.textoConfig(config, 'inicia_en');
        const terminaEn = this.textoConfig(config, 'termina_en');
        if (!recursoId || !iniciaEn || !terminaEn) {
          throw new DomainError(
            'RESERVA_CONFIGURACION_REQUERIDA',
            'Una reserva requiere recurso_id, inicia_en y termina_en.',
          );
        }
        if (item.cantidad !== 1) {
          throw new DomainError('RESERVA_CANTIDAD_INVALIDA', 'Una reserva admite cantidad 1.');
        }
        const desde = iniciaEn.slice(0, 10);
        const hasta = terminaEn.slice(0, 10);
        if (!/^\d{4}-\d{2}-\d{2}$/.test(desde) || !/^\d{4}-\d{2}-\d{2}$/.test(hasta)) {
          throw new DomainError('RESERVA_FECHAS_INVALIDAS', 'Usa fechas ISO con hora en la reserva.');
        }
        const cotizacion = await cotizarItemReserva({
          empresa_id: item.empresa_id,
          item_id: item.item_id,
          desde,
          hasta,
        });
        if (!cotizacion) throw new ServiceUnavailableError('catalog-service');
        const disponible = await verificarDisponibilidadReserva({
          empresa_id: item.empresa_id,
          recurso_id: recursoId,
          inicia_en: iniciaEn,
          termina_en: terminaEn,
        });
        if (disponible === false) {
          throw new ConflictError(
            'El recurso no esta disponible en ese rango.',
            { recurso_id: recursoId },
            'RESERVA_NO_DISPONIBLE',
          );
        }
        if (disponible === null) throw new ServiceUnavailableError('scheduling-service');
        item.precio_unitario_cents = cotizacion.total_cents;
        total = total.sumar(Money.desdeCentavos(cotizacion.total_cents).multiplicarPor(item.cantidad));
      }
    }

    // 2. control sincrono de viabilidad de stock (TC-04: respuesta 409 rapida).
    // Las cotizaciones no comprometen inventario.
    if (!esCotizacion) await this.verificarStock(items);

    // 3. venta a credito: la cuenta por cobrar se registra antes de la orden
    // (idempotente por order_id). Si la orden no se crea, se compensa.
    const orderId = randomUUID();
    const creadoEn = new Date().toISOString();
    if (metodoPago === 'credito') {
      const empresaId = items[0]?.empresa_id ?? '';
      const credito = await registrarCreditoOrden({
        empresa_id: empresaId,
        parte_id: comando.cliente_parte_id as string,
        order_id: orderId,
        monto_cents: total.centavos,
        concepto: `Pedido ${orderId.slice(0, 8)}`,
        vendedor_id: items[0]?.vendedor_id,
      });
      if (credito.estado === 'rechazado') {
        throw new ConflictError(
          'CREDITO_RECHAZADO',
          'La parte no tiene una cuenta de credito activa o cupo disponible.',
        );
      }
      if (credito.estado === 'error') {
        throw new ServiceUnavailableError('finance-service');
      }
    }

    // 4. transaccion: evento + outbox (proyecciones via trigger en orders.eventos)
    try {
      await this.pg.transaccion(async (client) => {
      await this.eventStore.appendEnTransaccion(client, orderId, 'OrderCreatedEvent', {
        order_id: orderId,
        comprador_id: clienteId,
        ...(comando.cliente_parte_id ? { cliente_parte_id: comando.cliente_parte_id } : {}),
        items,
        total_cents: total.centavos,
        estado: 'creada',
        tipo_orden: tipoOrden,
        metodo_pago: metodoPago,
        fulfillment_config: this.resumenFulfillment(items),
        creado_en: creadoEn,
      });
      const evento = {
        event_id: randomUUID(),
        tipo: EVENTOS.ORDER_CREATED,
        ocurrido_en: creadoEn,
        data: {
          order_id: orderId,
          comprador_id: clienteId,
          ...(comando.cliente_parte_id ? { cliente_parte_id: comando.cliente_parte_id } : {}),
          items,
          total_cents: total.centavos,
          estado: 'creada',
          tipo_orden: tipoOrden,
          metodo_pago: metodoPago,
          fulfillment_config: this.resumenFulfillment(items),
          comision_cents: esCotizacion ? 0 : total.comision(this.tasaComision).centavos,
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
    } catch (error) {
      if (metodoPago === 'credito') {
        await anularCreditoOrden(orderId).catch(() => undefined);
      }
      throw error;
    }

    const primerItem = items[0];
    const vista: OrderView = {
      id: orderId,
      vendedor_id: primerItem?.vendedor_id ?? '',
      comprador_id: clienteId,
      tienda_id: primerItem?.tienda_id ?? '',
      estado: 'creada',
      total_cents: total.centavos,
      comision_cents: esCotizacion ? 0 : total.comision(0.12).centavos,
      moneda: 'C$',
      items,
      direccion_envio: null,
      creado_en: creadoEn,
      actualizado_en: creadoEn,
      tipo_orden: tipoOrden,
      metodo_pago: metodoPago,
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
    eventoAdicional?: { tipo: string; data: Record<string, unknown> },
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
      // Evento derivado (p. ej. payment.procesado) en la MISMA transacción:
      // si falla el outbox, la transición también revierte y el reintento es
      // seguro (antes la orden quedaba pagada sin evento de pago).
      if (eventoAdicional) {
        const extra = {
          event_id: randomUUID(),
          tipo: eventoAdicional.tipo,
          ocurrido_en: new Date().toISOString(),
          data: eventoAdicional.data,
        };
        await client.query(
          `INSERT INTO orders.outbox (event_id, tipo, payload, estado, creado_en)
           VALUES ($1, $2, $3, 'pendiente', NOW())`,
          [extra.event_id, extra.tipo, JSON.stringify(extra)],
        );
      }
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

  /**
   * Resuelve precios por empresa en una llamada por lote. Los alquileres se
   * excluyen porque su precio se cotiza por rango de fechas (tarifas).
   */
  private async resolverPrecios(
    lineas: LineaAgrupada[],
    porId: Map<string, OfertaExterna>,
    comando: CreateOrderCommand,
  ): Promise<Map<string, { origen: string; precio_unitario_cents: number }>> {
    const resultado = new Map<string, { origen: string; precio_unitario_cents: number }>();
    const porEmpresa = new Map<string, { clave: string; item_id: string; cantidad: number }[]>();
    for (const linea of lineas) {
      const oferta = porId.get(linea.oferta_id);
      if (!oferta || oferta.item_type === 'alquiler') continue;
      const grupo = porEmpresa.get(oferta.empresa_id) ?? [];
      grupo.push({ clave: oferta.id, item_id: oferta.item_id, cantidad: linea.cantidad });
      porEmpresa.set(oferta.empresa_id, grupo);
    }

    for (const [empresaId, grupo] of porEmpresa) {
      const respuesta = await resolverPreciosItems({
        empresa_id: empresaId,
        lineas: grupo,
        ...(comando.lista_precio_codigo ? { lista_codigo: comando.lista_precio_codigo } : {}),
        ...(comando.segmento_codigo ? { segmento_codigo: comando.segmento_codigo } : {}),
      });
      if (respuesta.estado === 'no_encontrado') {
        throw new NotFoundError(
          'Lista de precios',
          comando.lista_precio_codigo ?? comando.segmento_codigo ?? '',
        );
      }
      if (respuesta.estado === 'error') {
        // Sin seleccion explicita se degrada al precio de la oferta (R-AR-2).
        if (comando.lista_precio_codigo || comando.segmento_codigo) {
          throw new ServiceUnavailableError('catalog-service');
        }
        continue;
      }
      for (const precio of respuesta.precios) {
        resultado.set(precio.clave, precio);
      }
    }
    return resultado;
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
    if (tipoOrden === 'reserva' && !['servicio', 'alquiler'].includes(itemType)) {
      throw new DomainError('TIPO_ITEM_INVALIDO', 'Las reservas aplican a servicios o alquileres.');
    }
    if (tipoOrden === 'reserva' && fulfillment !== 'onsite') {
      throw new DomainError('FULFILLMENT_INVALIDO', 'Una reserva requiere fulfillment onsite.');
    }
    return fulfillment;
  }

  private textoConfig(config: Record<string, unknown>, clave: string): string | undefined {
    const valor = config[clave];
    return typeof valor === 'string' && valor.trim() ? valor : undefined;
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

  private async validarCapacidades(
    items: ItemOrden[],
    tipoOrden: TipoOrden,
    metodoPago: 'simulado' | 'credito' = 'simulado',
  ): Promise<void> {
    const requeridas = new Set<string>();
    if (tipoOrden === 'cotizacion') requeridas.add('cotizaciones');
    if (metodoPago === 'credito') requeridas.add('ventas_credito');
    for (const item of items) {
      if (item.fulfillment === 'delivery') requeridas.add('fulfillment_delivery');
      if (item.fulfillment === 'pickup') requeridas.add('fulfillment_pickup');
      if (item.fulfillment === 'onsite') requeridas.add('fulfillment_onsite');
    }
    if (requeridas.size === 0) return;
    const empresas = [...new Set(items.map((item) => item.empresa_id))];
    // identity es la dueña de sus capacidades: se consulta por su superficie
    // interna en lugar de leer `identity.empresa_capacidades`.
    const filas = await capacidadesDeEmpresas(empresas, [...requeridas]);
    if (filas === null) throw new ServiceUnavailableError('identity-service');
    const activas = new Set(
      filas
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
        {
          headers: { 'x-internal-key': this.claveInterna },
          signal: AbortSignal.timeout(timeoutInternoMs()),
        },
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
        `${this.catalogUrl}/internal/items/lote?skus=${encodeURIComponent(skus.join(','))}`,
        {
          headers: { 'x-internal-key': this.claveInterna },
          signal: AbortSignal.timeout(timeoutInternoMs()),
        },
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
