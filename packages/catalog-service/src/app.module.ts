import { Module } from '@nestjs/common';
import { PgModule, RabbitModule, OutboxModule, MetricsModule } from '@core/shared';
import { ProductosModule } from './productos/productos.module';
import { InventarioModule } from './inventario/inventario.module';
import { PedidosConsumer } from './eventos/pedidos.consumer';
import { InternalController } from './internal/internal.controller';

@Module({
  imports: [PgModule, RabbitModule, OutboxModule, MetricsModule, ProductosModule, InventarioModule],
  providers: [PedidosConsumer],
  controllers: [InternalController],
})
export class AppModule {}