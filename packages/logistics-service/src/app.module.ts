import { Module } from '@nestjs/common';
import { PgModule, RabbitModule, OutboxModule, MetricsModule } from '@core/shared';
import { EnviosModule } from './envios/envios.module';
import {
  EstadoOrdenController,
  EnviosAdminController,
} from './envios/estado-orden.controller';

@Module({
  imports: [PgModule, RabbitModule, OutboxModule, MetricsModule, EnviosModule],
  controllers: [EstadoOrdenController, EnviosAdminController],
})
export class AppModule {}