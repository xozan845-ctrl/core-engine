import { Module } from '@nestjs/common';
import { CapacidadesController } from './capacidades.controller';
import { CapacidadesService } from './capacidades.service';

@Module({
  controllers: [CapacidadesController],
  providers: [CapacidadesService],
  exports: [CapacidadesService],
})
export class CapacidadesModule {}
