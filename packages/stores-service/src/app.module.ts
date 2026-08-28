import { Module } from '@nestjs/common';
import { PgModule, RabbitModule, OutboxModule, MetricsModule } from '@core/shared';
import { TiendasModule } from './tiendas/tiendas.module';
import { OfertasModule } from './ofertas/ofertas.module';
import { InternalController } from './internal/internal.controller';
import { StockConsumer } from './eventos/stock.consumer';

@Module({
  imports: [PgModule, RabbitModule, OutboxModule, MetricsModule, TiendasModule, OfertasModule],
  controllers: [InternalController],
  providers: [StockConsumer],
})
export class AppModule {}