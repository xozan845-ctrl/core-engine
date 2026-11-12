import { Module } from '@nestjs/common';
import { EmailModule, MetricsModule, PgModule, RabbitModule } from '@core/shared';
import { NotificacionesModule } from './notificaciones/notificaciones.module';

@Module({
  imports: [PgModule, RabbitModule, MetricsModule, EmailModule, NotificacionesModule],
})
export class AppModule {}
