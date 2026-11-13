import { Module } from '@nestjs/common';
import { InventarioService } from './inventario.service';
import { InventarioController } from './inventario.controller';
import { UbicacionesService } from './ubicaciones.service';
import { UbicacionesController } from './ubicaciones.controller';

@Module({
  providers: [InventarioService, UbicacionesService],
  exports: [InventarioService, UbicacionesService],
  controllers: [InventarioController, UbicacionesController],
})
export class InventarioModule {}