import { Body, Controller, Get, Param, Post, Query } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiQuery, ApiTags } from '@nestjs/swagger';
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

@ApiTags('Pedidos')
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
  @ApiOperation({ summary: 'Crear una orden desde el carrito (comprador)' })
  async crear(
    @Body() comando: CreateOrderCommand,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<OrderView> {
    return this.handler.ejecutar(comando, usuario.user_id);
  }

  /** GET /api/v1/orders/admin/todas — todas las ordenes (admin). */
  @Get('admin/todas')
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'Listar todas las órdenes (admin)' })
  @ApiQuery({ name: 'estado', required: false })
  @ApiQuery({ name: 'limite', required: false, type: Number })
  async todas(
    @Query('estado') estado?: string,
    @Query('limite') limite?: number,
  ): Promise<OrderView[]> {
    return this.views.listarTodo({ estado, limite });
  }

  /** GET /api/v1/orders/vendedor/mis-ventas — ordenes del vendedor autenticado. */
  @Get('vendedor/mis-ventas')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Órdenes del vendedor autenticado (mis ventas)' })
  @ApiQuery({ name: 'estado', required: false })
  async misVentas(
    @Query('estado') estado?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
  ): Promise<OrderView[]> {
    return this.views.listarDeVendedor(usuario?.user_id ?? '', estado);
  }

  /** GET /api/v1/orders/vendedor/comisiones — comisiones por periodo (vendedor/admin). */
  @Get('vendedor/comisiones')
  @Roles(ROLES.VENDEDOR, ROLES.ADMIN)
  @ApiOperation({ summary: 'Comisiones por periodo (vendedor/admin)' })
  @ApiQuery({ name: 'periodo', required: false })
  @ApiQuery({ name: 'vendedorId', required: false, description: 'Solo admin' })
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
  @ApiOperation({ summary: 'Órdenes de una tienda (vendedor dueño o admin)' })
  @ApiParam({ name: 'tiendaId', description: 'Id de la tienda' })
  @ApiQuery({ name: 'estado', required: false })
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
  @ApiOperation({ summary: 'Historial de órdenes del comprador autenticado' })
  @ApiQuery({ name: 'estado', required: false })
  async misOrdenes(
    @Query('estado') estado?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
  ): Promise<OrderView[]> {
    return this.views.listarDeCliente(usuario?.user_id ?? '', estado);
  }

  /** GET /api/v1/orders/:id — estado de la orden (JWT; solo el dueno o admin). */
  @Get(':id')
  @Roles(ROLES.COMPRADOR, ROLES.VENDEDOR, ROLES.ADMIN)
  @ApiOperation({ summary: 'Estado/detalle de una orden (dueño o admin)' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  async detalle(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<OrderView> {
    return this.pedidos.detallePara(id, usuario);
  }

  /** GET /api/v1/orders/:id/timeline — historial de eventos de la orden. */
  @Get(':id/timeline')
  @Roles(ROLES.COMPRADOR, ROLES.VENDEDOR, ROLES.ADMIN)
  @ApiOperation({ summary: 'Timeline de eventos de la orden (Event Sourcing)' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  async timeline(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    const orden = await this.pedidos.detallePara(id, usuario);
    return this.views.obtenerTimeline(orden.id);
  }
}
