import { Module } from '@nestjs/common';
import { PgModule, RabbitModule, OutboxModule, MetricsModule } from '@core/shared';
import { ComisionesModule } from './comisiones/comisiones.module';
import { LiquidacionesModule } from './liquidaciones/liquidaciones.module';
import { VentasModule } from './ventas/ventas.module';

@Module({
  imports: [
    PgModule,
    RabbitModule,
    OutboxModule,
    MetricsModule,
    ComisionesModule,
    LiquidacionesModule,
    VentasModule,
  ],
})
export class AppModule {}