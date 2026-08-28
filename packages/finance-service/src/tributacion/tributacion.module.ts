import { Module } from '@nestjs/common';
import { TributacionService } from './tributacion.service';
import { TributacionController } from './tributacion.controller';
import { ContabilidadModule } from '../contabilidad/contabilidad.module';

@Module({
  imports: [ContabilidadModule],
  controllers: [TributacionController],
  providers: [TributacionService],
  exports: [TributacionService],
})
export class TributacionModule {}