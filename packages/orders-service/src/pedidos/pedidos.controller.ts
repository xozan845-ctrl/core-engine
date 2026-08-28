import { Body, Controller, Get, Param, Post, Query } from '@nestjs/common';
import {
  Roles,
  ROLES,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import { CreateOrderCommand } from './commands/create-order.command';
import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import { OrderViewRepository, OrderView, ComisionVista } from './queries/order-view.repository';
import { PedidosService } from './pedidos.service';

@Controller('api/v1/orders')
export class PedidosController {
  constructor(
    private readonly handler: CreateOrderCommandHandler,
    private readonly views: OrderViewRepository,
    private readonly pedidos: PedidosService,
  ) {}

  /** POST /api/v1/orders — crear orden desde el carrito (JWT comprador, Tabla 21). */
  @Post()
  @Roles(ROLES.COMPRADOR)
  async crear(
    @Body() comando: CreateOrderCommand,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<OrderView> {
    return this.handler.ejecutar(comando, usuario.user_id);
  }

  /** GET /api/v1/orders/admin/todas — todas las ordenes (admin). */
  @Get('admin/todas')
  @Roles(ROLES.ADMIN)
  async todas(
    @Query('estado') estado?: string,
    @Query('limite') limite?: number,
  ): Promise<OrderView[]> {
    return this.views.listarTodo({ estado, limite });
  }

  /** GET /api/v1/orders/vendedor/mis-ventas — ordenes del vendedor autenticado. */
  @Get('vendedor/mis-ventas')
  @Roles(ROLES.VENDEDOR)
  async misVentas(
    @Query('estado') estado?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
  ): Promise<OrderView[]> {
    return this.views.listarDeVendedor(usuario?.user_id ?? '', estado);
  }

  /** GET /api/v1/orders/vendedor/comisiones — comisiones por periodo (vendedor/admin). */
  @Get('vendedor/comisiones')
  @Roles(ROLES.VENDEDOR, ROLES.ADMIN)
  async comisiones(
    @Query('periodo') periodo?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
    @Query('vendedorId') vendedorId?: string,
  ): Promise<ComisionVista[]> {
    const targetVendedor = usuario?.rol === 'admin' ? (vendedorId ?? usuario?.user_id) : usuario?.user_id;
    return this.views.obtenerComisiones(targetVendedor ?? '', periodo);
  }

  /** GET /api/v1/orders/tienda/:tiendaId — ordenes de una tienda (vendedor dueño o admin). */
  @Get('tienda/:tiendaId')
  @Roles(ROLES.VENDEDOR, ROLES.ADMIN)
  async porTienda(
    @Param('tiendaId') tiendaId: string,
    @Query('estado') estado?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
  ): Promise<OrderView[]> {
    return this.views.listarDeTienda(
      tiendaId,
      estado,
      usuario?.rol === 'vendedor' ? usuario.user_id : undefined,
    );
  }

  /** GET /api/v1/orders — historial del comprador autenticado. */
  @Get()
  @Roles(ROLES.COMPRADOR)
  async misOrdenes(
    @Query('estado') estado?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
  ): Promise<OrderView[]> {
    return this.views.listarDeCliente(usuario?.user_id ?? '', estado);
  }

  /** GET /api/v1/orders/:id — estado de la orden (JWT; solo el dueno o admin). */
  @Get(':id')
  @Roles(ROLES.COMPRADOR, ROLES.VENDEDOR, ROLES.ADMIN)
  async detalle(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<OrderView> {
    return this.pedidos.detallePara(id, usuario);
  }

  /** GET /api/v1/orders/:id/timeline — historial de eventos de la orden. */
  @Get(':id/timeline')
  @Roles(ROLES.COMPRADOR, ROLES.VENDEDOR, ROLES.ADMIN)
  async timeline(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    const orden = await this.pedidos.detallePara(id, usuario);
    return this.views.obtenerTimeline(orden.id);
  }
}