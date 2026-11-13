import { Module } from '@nestjs/common';
import { PedidosController } from './pedidos.controller';
import { PedidosService } from './pedidos.service';
import { OrderEventStore } from './repositories/order-event-store';
import { OrderViewRepository } from './queries/order-view.repository';
import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import { CarritoService } from './carrito/carrito.service';
import { CarritoController } from './carrito/carrito.controller';
import { EntregasDigitalesService } from './entregas-digitales.service';
import { ContratosService } from './contratos.service';
import { ContratosController } from './contratos.controller';
import { SuscripcionesService } from './suscripciones.service';
import { SuscripcionesController } from './suscripciones.controller';
import { SuscripcionesScheduler } from './suscripciones.scheduler';
import { SalonService } from './salon.service';
import { SalonController } from './salon.controller';
import { EntregasDigitalesController } from './entregas-digitales.controller';

@Module({
  controllers: [PedidosController, CarritoController, EntregasDigitalesController, ContratosController, SuscripcionesController, SalonController],
  providers: [
    PedidosService,
    OrderEventStore,
    OrderViewRepository,
    CreateOrderCommandHandler,
    CarritoService,
    EntregasDigitalesService,
    ContratosService,
    SuscripcionesService,
    SuscripcionesScheduler,
    SalonService,
  ],
  exports: [
    PedidosService,
    OrderEventStore,
    OrderViewRepository,
    CreateOrderCommandHandler,
    CarritoService,
    EntregasDigitalesService,
    ContratosService,
    SuscripcionesService,
    SuscripcionesScheduler,
    SalonService,
  ],
})
export class PedidosModule {}