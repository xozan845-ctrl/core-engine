import { Module } from '@nestjs/common';
import { PedidosController } from './pedidos.controller';
import { PedidosService } from './pedidos.service';
import { OrderEventStore } from './repositories/order-event-store';
import { OrderViewRepository } from './queries/order-view.repository';
import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import { CarritoService } from './carrito/carrito.service';
import { CarritoController } from './carrito/carrito.controller';

@Module({
  controllers: [PedidosController, CarritoController],
  providers: [
    PedidosService,
    OrderEventStore,
    OrderViewRepository,
    CreateOrderCommandHandler,
    CarritoService,
  ],
  exports: [
    PedidosService,
    OrderEventStore,
    OrderViewRepository,
    CreateOrderCommandHandler,
    CarritoService,
  ],
})
export class PedidosModule {}