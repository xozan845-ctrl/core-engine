import { Module } from '@nestjs/common';
import { PgModule, RabbitModule, OutboxModule, MetricsModule } from '@core/shared';
import { EnviosModule } from './envios/envios.module';
import {
  EstadoOrdenController,
  EnviosAdminController,
  EnviosEmpresaController,
  EnviosFlotaController,
} from './envios/estado-orden.controller';

@Module({
  imports: [PgModule, RabbitModule, OutboxModule, MetricsModule, EnviosModule],
  controllers: [EstadoOrdenController, EnviosAdminController, EnviosEmpresaController, EnviosFlotaController],
})
export class AppModule {}
