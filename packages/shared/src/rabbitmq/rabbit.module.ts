import { Global, Module } from '@nestjs/common';
import { RabbitService } from './rabbit.service';

/**
 * Provee la conexion al broker (RabbitMQ / AD-02) y un publisher con confirmacion.
 */
@Global()
@Module({
  providers: [RabbitService],
  exports: [RabbitService],
})
export class RabbitModule {}