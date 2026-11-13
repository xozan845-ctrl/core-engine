import { Module } from '@nestjs/common';
import { PedidosController } from './pedidos.controller';
import { PedidosService } from './pedidos.service';
import { OrderEventStore } from './repositories/order-event-store';
import { OrderViewRepository } from './queries/order-view.repository';
import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import { CarritoService } from './carrito/carrito.service';
import { CarritoController } from './carrito/carrito.controller';
import { EntregasDigitalesService } from './entregas-digitales.service';
import { EntregasDigitalesController } from './entregas-digitales.controller';

@Module({
  controllers: [PedidosController, CarritoController, EntregasDigitalesController],
  providers: [
    PedidosService,
    OrderEventStore,
    OrderViewRepository,
    CreateOrderCommandHandler,
    CarritoService,
    EntregasDigitalesService,
  ],
  exports: [
    PedidosService,
    OrderEventStore,
    OrderViewRepository,
    CreateOrderCommandHandler,
    CarritoService,
    EntregasDigitalesService,
  ],
})
export class PedidosModule {}