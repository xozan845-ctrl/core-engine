import { Module } from '@nestjs/common';
import { ContabilidadService } from './contabilidad.service';
import { ContabilidadController } from './contabilidad.controller';

@Module({
  controllers: [ContabilidadController],
  providers: [ContabilidadService],
  exports: [ContabilidadService],
})
export class ContabilidadModule {}