import { Module } from '@nestjs/common';
import { PgModule, RabbitModule, OutboxModule, MetricsModule } from '@core/shared';
import { ContabilidadModule } from './contabilidad/contabilidad.module';
import { FacturacionModule } from './facturacion/facturacion.module';
import { TributacionModule } from './tributacion/tributacion.module';
import { FinanzasModule } from './finanzas/finanzas.module';
import { EventosModule } from './eventos/eventos.module';
import { InternoController } from './interno.controller';

@Module({
  imports: [
    PgModule,
    RabbitModule,
    OutboxModule,
    MetricsModule,
    ContabilidadModule,
    FacturacionModule,
    TributacionModule,
    FinanzasModule,
    EventosModule,
  ],
  controllers: [InternoController],
})
export class AppModule {}