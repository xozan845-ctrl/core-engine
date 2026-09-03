import { Injectable, Logger } from '@nestjs/common';
import { Cron, CronExpression } from '@nestjs/schedule';
import { PgService, MetricsService } from '@core/shared';

const GAUGE_REFRESH_MS = 'intelligence_view_refresh_duration_ms';

@Injectable()
export class ViewRefresherService {
  private readonly logger = new Logger(ViewRefresherService.name);

  constructor(
    private readonly pg: PgService,
    private readonly metrics: MetricsService,
  ) {}

  @Cron(CronExpression.EVERY_30_SECONDS)
  async refreshMaterializedViews() {
    this.logger.debug('Iniciando refresco concurrente de vistas materializadas...');
    const start = Date.now();

    try {
      // Usamos CONCURRENTLY para que las vistas no se bloqueen durante la lectura.
      // Importante: PostgreSQL requiere que las vistas materializadas tengan al menos un UNIQUE INDEX
      // para poder usar CONCURRENTLY, los cuales creamos en 05_materialized_views.sql
      
      await this.pg.query('REFRESH MATERIALIZED VIEW CONCURRENTLY intelligence.mv_resumen_ejecutivo');
      await this.pg.query('REFRESH MATERIALIZED VIEW CONCURRENTLY intelligence.mv_tendencias');

      const ms = Date.now() - start;
      this.metrics.registrarGauge(GAUGE_REFRESH_MS, ms);
      this.logger.debug(`Refresco concurrente completado en ${ms}ms`);
    } catch (error: any) {
      this.logger.error(`Error refrescando vistas materializadas: ${error.message}`, error.stack);
    }
  }
}
