import { Body, Controller, Get, Param, Post, Query } from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiOkResponse, ApiOperation, ApiParam, ApiQuery, ApiTags } from '@nestjs/swagger';
import {
  DomainError,
  Permisos,
  Roles,
  ROLES,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import { CreateOrderCommand } from './commands/create-order.command';
import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import { OrderViewRepository, OrderView } from './queries/order-view.repository';
import { PedidosService } from './pedidos.service';
import {
  OrderViewResponseDto,
  TimelineEventoResponseDto,
} from './orders.response.dto';

@ApiTags('Pedidos')
@ApiBearerAuth()
@Controller('api/v2/orders')
export class PedidosController {
  constructor(
    private readonly handler: CreateOrderCommandHandler,
    private readonly views: OrderViewRepository,
    private readonly pedidos: PedidosService,
  ) {}

  /** POST /api/v2/orders — crear orden desde el carrito (JWT comprador, Tabla 21). */
  @Post()
  @Roles(ROLES.COMPRADOR)
  @ApiOperation({ summary: 'Crear una orden desde el carrito (comprador)' })
  @ApiCreatedResponse({ type: OrderViewResponseDto, description: 'Orden creada' })
  async crear(
    @Body() comando: CreateOrderCommand,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<OrderView> {
    return this.handler.ejecutar(comando, usuario.user_id);
  }

  /** GET /api/v2/orders/admin/todas — todas las ordenes (admin). */
  @Get('admin/todas')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'Listar todas las órdenes (admin)' })
  @ApiQuery({ name: 'estado', required: false })
  @ApiQuery({ name: 'limite', required: false, type: Number })
  @ApiOkResponse({ type: [OrderViewResponseDto], description: 'Órdenes registradas' })
  async todas(
    @Query('estado') estado?: string,
    @Query('limite') limite?: number,
  ): Promise<OrderView[]> {
    return this.views.listarTodo({ estado, limite });
  }

  /** GET /api/v2/orders/vendedor/mis-ventas — ordenes del vendedor autenticado. */
  @Get('vendedor/mis-ventas')
  @Permisos('ordenes.leer')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Órdenes del vendedor autenticado (mis ventas)' })
  @ApiQuery({ name: 'estado', required: false })
  @ApiOkResponse({ type: [OrderViewResponseDto], description: 'Ventas del vendedor' })
  async misVentas(
    @Query('estado') estado?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
  ): Promise<OrderView[]> {
    const ordenes = await this.views.listarDeVendedor(usuario?.user_id ?? '', estado);
    return ordenes.map((orden) => this.pedidos.paraVendedor(orden, usuario?.user_id ?? ''));
  }

  /** GET /api/v2/orders/tienda/:tiendaId — ordenes de una tienda (vendedor dueño o admin). */
  @Get('tienda/:tiendaId')
  @Permisos('ordenes.leer')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR, ROLES.ADMIN)
  @ApiOperation({ summary: 'Órdenes de una tienda (vendedor dueño, empresario de la empresa o admin)' })
  @ApiParam({ name: 'tiendaId', description: 'Id de la tienda' })
  @ApiQuery({ name: 'estado', required: false })
  @ApiOkResponse({ type: [OrderViewResponseDto], description: 'Órdenes de la tienda' })
  async porTienda(
    @Param('tiendaId') tiendaId: string,
    @Query('estado') estado?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
  ): Promise<OrderView[]> {
    const esPersonalEmpresa = usuario?.rol === 'empresario' || Boolean(usuario?.rol_empresa_id);
    const empresaId = esPersonalEmpresa ? usuario?.tenant_id : undefined;
    if (esPersonalEmpresa && !empresaId) {
      throw new DomainError('TENANT_REQUERIDO', 'Se requiere una empresa activa.');
    }
    const ordenes = await this.views.listarDeTienda(
      tiendaId,
      estado,
      usuario?.rol === 'vendedor' || usuario?.rol_empresa_id ? usuario.user_id : undefined,
      empresaId,
    );
    return this.proyectar(ordenes, usuario, empresaId);
  }

  /** GET /api/v2/orders/empresa — ordenes de la empresa activa (empresario/admin). */
  @Get('empresa')
  @Permisos('ordenes.leer')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN)
  @ApiOperation({ summary: 'Órdenes de una empresa (empresario de la empresa o admin)' })
  @ApiQuery({
    name: 'empresaId',
    required: false,
    description: 'Solo admin/super_admin; el empresario usa su empresa activa',
  })
  @ApiQuery({ name: 'tiendaId', required: false })
  @ApiQuery({ name: 'estado', required: false })
  @ApiQuery({ name: 'limite', required: false, type: Number })
  @ApiOkResponse({ type: [OrderViewResponseDto], description: 'Órdenes de la empresa' })
  async porEmpresa(
    @Query('empresaId') empresaId?: string,
    @Query('tiendaId') tiendaId?: string,
    @Query('estado') estado?: string,
    @Query('limite') limite?: number,
    @UsuarioActual() usuario?: UsuarioContexto,
  ): Promise<OrderView[]> {
    let objetivo = empresaId;
    if (usuario?.rol === 'empresario' || usuario?.rol_empresa_id) {
      if (!usuario.tenant_id) {
        throw new DomainError('TENANT_REQUERIDO', 'Se requiere una empresa activa.');
      }
      objetivo = usuario.tenant_id;
    }
    if (!objetivo) {
      throw new DomainError('EMPRESA_REQUERIDA', 'Indique la empresa a consultar.');
    }
    const ordenes = await this.views.listarDeEmpresa(objetivo, {
      estado,
      tiendaId,
      limite,
    });
    return this.proyectar(ordenes, usuario, objetivo);
  }

  /** GET /api/v2/orders — historial del comprador autenticado. */
  @Get()
  @Roles(ROLES.COMPRADOR)
  @ApiOperation({ summary: 'Historial de órdenes del comprador autenticado' })
  @ApiQuery({ name: 'estado', required: false })
  @ApiOkResponse({ type: [OrderViewResponseDto], description: 'Órdenes del comprador' })
  async misOrdenes(
    @Query('estado') estado?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
  ): Promise<OrderView[]> {
    return this.views.listarDeCliente(usuario?.user_id ?? '', estado);
  }

  /** GET /api/v2/orders/:id — estado de la orden (JWT; solo el dueno o admin). */
  @Get(':id')
  @Permisos('ordenes.leer')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.COMPRADOR, ROLES.VENDEDOR, ROLES.ADMIN)
  @ApiOperation({ summary: 'Estado/detalle de una orden (dueño o admin)' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  @ApiOkResponse({ type: OrderViewResponseDto, description: 'Detalle de la orden' })
  async detalle(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<OrderView> {
    return this.pedidos.detallePara(id, usuario);
  }

  /** GET /api/v2/orders/:id/timeline — historial de eventos de la orden. */
  @Get(':id/timeline')
  @Permisos('ordenes.leer')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.COMPRADOR, ROLES.VENDEDOR, ROLES.ADMIN)
  @ApiOperation({ summary: 'Timeline de eventos de la orden (Event Sourcing)' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  @ApiOkResponse({ type: [TimelineEventoResponseDto], description: 'Timeline de la orden' })
  async timeline(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    const orden = await this.pedidos.detallePara(id, usuario);
    const eventos = await this.views.obtenerTimeline(orden.id);
    return this.pedidos.timelineVisible(eventos, orden.items);
  }

  private proyectar(
    ordenes: OrderView[],
    usuario?: UsuarioContexto,
    empresaId?: string,
  ): OrderView[] {
    if ((usuario?.rol === 'empresario' || usuario?.rol_empresa_id) && empresaId) {
      return ordenes.map((orden) => this.pedidos.paraEmpresa(orden, empresaId));
    }
    if ((usuario?.rol === 'vendedor' || usuario?.rol_empresa_id) && usuario.user_id) {
      return ordenes.map((orden) => this.pedidos.paraVendedor(orden, usuario.user_id));
    }
    return ordenes;
  }
}
