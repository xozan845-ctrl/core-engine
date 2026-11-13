import { Module } from '@nestjs/common';
import { ScheduleModule } from '@nestjs/schedule';
import {
  PgModule,
  RabbitModule,
  OutboxModule,
  MetricsModule,
} from '@core/shared';
import { PedidosModule } from './pedidos/pedidos.module';
import { PedidosConsumer } from './pedidos/pedidos.consumer';
import { InternoController } from './pedidos/interno.controller';

@Module({
  imports: [ScheduleModule.forRoot(), PgModule, RabbitModule, OutboxModule, MetricsModule, PedidosModule],
  providers: [PedidosConsumer],
  controllers: [InternoController],
})
export class AppModule {}