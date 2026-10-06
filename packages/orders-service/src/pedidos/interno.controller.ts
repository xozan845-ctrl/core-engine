import { Controller, Get, Param, Post, Query, UseGuards } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiQuery, ApiTags } from '@nestjs/swagger';
import {
  ClaveInternaGuard,
  NotFoundError,
} from '@core/shared';
import { OrderViewRepository, OrderView } from './queries/order-view.repository';
import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import { OrderEventStore } from './repositories/order-event-store';
import { PedidosService } from './pedidos.service';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna:
 * transicion de estado (logistica), lectura para reportes y replay de eventos.
 */
@ApiTags('Pedidos (interno)')
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternoController {
  constructor(
    private readonly handler: CreateOrderCommandHandler,
    private readonly views: OrderViewRepository,
    private readonly eventStore: OrderEventStore,
    private readonly pedidos: PedidosService,
  ) {}

  /** PATCH logica -> orders: avanza el ciclo de vida (Tabla 13). */
  @Post('orders/:id/transicion')
  @ApiOperation({ summary: 'Avanzar el ciclo de vida de la orden (logística→orders)' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  @ApiQuery({ name: 'estado', description: 'Estado objetivo' })
  @ApiQuery({ name: 'motivo', required: false })
  async transicion(
    @Param('id') id: string,
    @Query('estado') estado: string,
    @Query('motivo') motivo?: string,
  ): Promise<OrderView> {
    if (!estado) throw new NotFoundError('Estado', 'requerido');
    return this.handler.transicionar(id, estado, motivo, 'logistica');
  }

  /** Lectura completa para reportes (admin) y comisiones. */
  @Get('orders/:id')
  @ApiOperation({ summary: 'Lectura completa de una orden (reportes/comisiones)' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  async orden(@Param('id') id: string): Promise<OrderView> {
    const orden = await this.views.encontrar(id);
    if (!orden) throw new NotFoundError('Orden', id);
    return orden;
  }

  @Get('orders')
  @ApiOperation({ summary: 'Listar órdenes (servicio→servicio)' })
  @ApiQuery({ name: 'estado', required: false })
  async listar(@Query('estado') estado?: string): Promise<OrderView[]> {
    return this.views.listarTodo(estado ? { estado } : {});
  }

  /** Replay de Event Sourcing: reconstruye la proyeccion desde la historia. */
  @Post('orders/:id/reproyectar')
  @ApiOperation({ summary: 'Reproyectar una orden desde su historia (Event Sourcing)' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  async reproyectar(@Param('id') id: string): Promise<OrderView | null> {
    return this.handler.reproyectar(id);
  }

  /** Auditoria: historia inmutable de eventos de una orden. */
  @Get('orders/:id/historia')
  @ApiOperation({ summary: 'Historia inmutable de eventos de una orden' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  async historia(@Param('id') id: string) {
    const historia = await this.eventStore.historiaDe(id);
    if (historia.length === 0) throw new NotFoundError('Orden', id);
    return historia.map((e) => ({
      event_id: e.event_id,
      aggregate_id: e.aggregate_id,
      tipo: e.tipo,
      payload: e.payload,
      version: e.version,
      creado_en: e.creado_en,
    }));
  }
}
