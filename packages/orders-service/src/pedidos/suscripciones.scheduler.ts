import { Injectable } from '@nestjs/common';
import { Cron, CronExpression } from '@nestjs/schedule';
import { Logger } from '@core/shared';
import { SuscripcionesService } from './suscripciones.service';

/** Renueva cada hora las suscripciones vencidas (idempotente por reclamacion). */
@Injectable()
export class SuscripcionesScheduler {
  private readonly logger = Logger.create('orders.suscripciones');

  constructor(private readonly suscripciones: SuscripcionesService) {}

  @Cron(CronExpression.EVERY_HOUR)
  async renovarVencidas(): Promise<void> {
    const renovadas = await this.suscripciones.renovarVencidas();
    if (renovadas > 0) {
      this.logger.info({ msg: 'Suscripciones renovadas', renovadas });
    }
  }
}
