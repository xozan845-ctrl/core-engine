import { Module } from '@nestjs/common';
import { ProyeccionesService } from './proyecciones.service';
import { KpisService } from './kpis.service';
import { AlertasService } from './alertas.service';
import { FinanzasController } from './finanzas.controller';
import { VendedorFiscalController } from './vendedor-fiscal.controller';
import { TributacionModule } from '../tributacion/tributacion.module';

@Module({
  imports: [TributacionModule],
  controllers: [FinanzasController, VendedorFiscalController],
  providers: [ProyeccionesService, KpisService, AlertasService],
  exports: [KpisService],
})
export class FinanzasModule {}