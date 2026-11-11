import { Module } from '@nestjs/common';
import { PgModule } from '@core/shared';
import { ItemsController } from './items.controller';
import { ItemsService } from './items.service';
import { TarifasController } from './tarifas.controller';
import { TarifasService } from './tarifas.service';
import { ComponentesController } from './componentes.controller';
import { ComponentesService } from './componentes.service';

@Module({
  imports: [PgModule],
  controllers: [ItemsController, TarifasController, ComponentesController],
  providers: [ItemsService, TarifasService, ComponentesService],
  exports: [ItemsService, TarifasService, ComponentesService],
})
export class ItemsModule {}
