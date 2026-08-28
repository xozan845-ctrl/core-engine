import { Module } from '@nestjs/common';
import { ComisionesService } from './comisiones.service';
import { PedidosConsumer } from './pedidos.consumer';

@Module({
  providers: [ComisionesService, PedidosConsumer],
  exports: [ComisionesService],
})
export class ComisionesModule {}