import { Module } from '@nestjs/common';
import { EventsConsumer } from './events.consumer';
import { MetricasService } from './metricas.service';
import { ContabilidadModule } from '../contabilidad/contabilidad.module';

@Module({
  imports: [ContabilidadModule],
  providers: [EventsConsumer, MetricasService],
  exports: [MetricasService],
})
export class EventosModule {}