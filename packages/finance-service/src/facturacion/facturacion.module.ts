import { Module } from '@nestjs/common';
import { FacturacionService } from './facturacion.service';
import { FacturacionController } from './facturacion.controller';
import { ContabilidadModule } from '../contabilidad/contabilidad.module';

@Module({
  imports: [ContabilidadModule],
  controllers: [FacturacionController],
  providers: [FacturacionService],
  exports: [FacturacionService],
})
export class FacturacionModule {}