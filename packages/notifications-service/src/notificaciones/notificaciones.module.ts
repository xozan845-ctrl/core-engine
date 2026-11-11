import { Module } from '@nestjs/common';
import { ClientesService } from './clientes.service';
import { NotificacionesService } from './notificaciones.service';
import { PedidosConsumer } from './pedidos.consumer';

@Module({
  providers: [ClientesService, NotificacionesService, PedidosConsumer],
})
export class NotificacionesModule {}
