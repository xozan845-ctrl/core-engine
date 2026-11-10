import { Module } from '@nestjs/common';
import { MetricsModule, OutboxModule, PgModule, RabbitModule } from '@core/shared';
import { SchedulingModule } from './scheduling/scheduling.module';

@Module({
  imports: [PgModule, RabbitModule, OutboxModule, MetricsModule, SchedulingModule],
})
export class AppModule {}
