import { Injectable } from '@nestjs/common';
import {
  ForbiddenError,
  ItemOrden,
  Money,
  NotFoundError,
  UsuarioContexto,
} from '@core/shared';
import {
  OrderViewRepository,
  OrderView,
  TimelineEvento,
} from './queries/order-view.repository';

const TASA_COMISION = 0.12;

/**
 * Servicio de consulta de pedidos: aplica el control de acceso por dominio
 * (Tabla 15: comprador solo sus compras, admin lectura global; el vendedor
 * accede a sus ventas y el empresario a las líneas de su empresa).
 *
 * Una orden puede contener líneas de varias empresas o vendedores. Por eso
 * el acceso no basta con comprobar que la orden tenga una línea propia: la
 * respuesta debe recortarse a las líneas visibles para el actor.
 */
@Injectable()
export class PedidosService {
  constructor(private readonly views: OrderViewRepository) {}

  async detallePara(id: string, usuario: UsuarioContexto): Promise<OrderView> {
    const orden = await this.views.encontrar(id);
    if (!orden) throw new NotFoundError('Orden', id);
    if (usuario.rol === 'admin' || usuario.rol === 'super_admin') return orden;
    if (usuario.rol === 'comprador' && orden.comprador_id === usuario.user_id) return orden;
    if (usuario.rol === 'vendedor') {
      const items = orden.items.filter((i) => i.vendedor_id === usuario.user_id);
      if (items.length > 0) return this.recortar(orden, items);
    }
    if (usuario.rol === 'empresario' && usuario.tenant_id) {
      const items = orden.items.filter((i) => i.empresa_id === usuario.tenant_id);
      if (items.length > 0) return this.recortar(orden, items);
    }
    throw new ForbiddenError('No tiene acceso a esta orden.');
  }

  /** Recorta una orden a las líneas de la empresa activa del empresario. */
  paraEmpresa(orden: OrderView, empresaId: string): OrderView {
    return this.recortar(
      orden,
      orden.items.filter((i) => i.empresa_id === empresaId),
    );
  }

  /** Recorta una orden a las líneas del vendedor autenticado. */
  paraVendedor(orden: OrderView, vendedorId: string): OrderView {
    return this.recortar(
      orden,
      orden.items.filter((i) => i.vendedor_id === vendedorId),
    );
  }

  /**
   * Oculta del timeline las líneas que no pertenecen al actor. Los eventos
   * `OrderCreatedEvent` incluyen el snapshot completo de la orden; devolver
   * ese payload sin filtrar anularía el recorte del detalle.
   */
  timelineVisible(
    eventos: TimelineEvento[],
    itemsVisibles: ItemOrden[],
  ): TimelineEvento[] {
    const ofertas = new Set(itemsVisibles.map((i) => i.oferta_id));
    return eventos.map((evento) => {
      const payload = evento.payload;
      if (!payload || typeof payload !== 'object' || !Array.isArray((payload as { items?: unknown }).items)) {
        return evento;
      }
      const datos = payload as { items: ItemOrden[] } & Record<string, unknown>;
      const items = datos.items.filter((i) => ofertas.has(i.oferta_id));
      return {
        ...evento,
        payload: {
          ...datos,
          items,
          total_cents: this.totalDe(items),
        },
      };
    });
  }

  private recortar(orden: OrderView, items: ItemOrden[]): OrderView {
    if (items.length === 0) {
      throw new ForbiddenError('No tiene acceso a esta orden.');
    }
    const total = this.totalDe(items);
    return {
      ...orden,
      vendedor_id: items[0].vendedor_id,
      tienda_id: items[0].tienda_id,
      total_cents: total,
      comision_cents: Money.desdeCentavos(total).comision(TASA_COMISION).centavos,
      items,
    };
  }

  private totalDe(items: ItemOrden[]): number {
    return items.reduce(
      (total, item) => total + item.precio_unitario_cents * item.cantidad,
      0,
    );
  }
}