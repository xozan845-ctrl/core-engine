import { Injectable } from '@nestjs/common';
import {
  ForbiddenError,
  NotFoundError,
  UsuarioContexto,
} from '@core/shared';
import { OrderViewRepository, OrderView } from './queries/order-view.repository';

/**
 * Servicio de consulta de pedidos: aplica el control de acceso por dominio
 * (Tabla 15: comprador solo sus compras, admin lectura global; el vendedor
 * accede a sus ventas por los items enriquecidos).
 */
@Injectable()
export class PedidosService {
  constructor(private readonly views: OrderViewRepository) {}

  async detallePara(id: string, usuario: UsuarioContexto): Promise<OrderView> {
    const orden = await this.views.encontrar(id);
    if (!orden) throw new NotFoundError('Orden', id);
    if (usuario.rol === 'admin') return orden;
    if (usuario.rol === 'comprador' && orden.comprador_id === usuario.user_id) return orden;
    if (
      usuario.rol === 'vendedor' &&
      orden.items.some((i) => i.vendedor_id === usuario.user_id)
    ) {
      return orden;
    }
    throw new ForbiddenError('No tiene acceso a esta orden.');
  }
}