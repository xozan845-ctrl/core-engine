import { Module } from '@nestjs/common';
import { EnviosService } from './envios.service';
import { PedidosConsumer } from './pedidos.consumer';

@Module({
  providers: [EnviosService, PedidosConsumer],
  exports: [EnviosService],
})
export class EnviosModule {}