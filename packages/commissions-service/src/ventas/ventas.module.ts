import { Module } from '@nestjs/common';
import { VentasController, VentasEmpresaController } from './ventas.controller';
import { ReportesController } from './reportes.controller';
import { VentasService } from './ventas.service';
import { ReportesService } from './reportes.service';

@Module({
  controllers: [VentasController, VentasEmpresaController, ReportesController],
  providers: [VentasService, ReportesService],
})
export class VentasModule {}