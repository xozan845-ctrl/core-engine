import { Global, Module } from '@nestjs/common';
import { OutboxService } from './outbox.service';

/**
 * Patron Outbox (ADR-03): los eventos se persisten en la misma transaccion de
 * negocio y un publicador los reenvia de forma fiable al bus.
 */
@Global()
@Module({
  providers: [OutboxService],
  exports: [OutboxService],
})
export class OutboxModule {}