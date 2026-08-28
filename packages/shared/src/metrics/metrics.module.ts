import { Global, Module } from '@nestjs/common';
import { MetricsService } from './metrics.service';

/**
 * Observabilidad (doc 5.3): metricas Prometheus con latencia y contadores por
 * servicio. El endpoint /metrics lo expone cada microservicio.
 */
@Global()
@Module({
  providers: [MetricsService],
  exports: [MetricsService],
})
export class MetricsModule {}