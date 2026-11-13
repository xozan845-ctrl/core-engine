import { Module } from '@nestjs/common';
import { PgModule } from '@core/shared';
import { ItemsController } from './items.controller';
import { ItemsService } from './items.service';
import { TarifasController } from './tarifas.controller';
import { TarifasService } from './tarifas.service';
import { ComponentesController } from './componentes.controller';
import { ComponentesService } from './componentes.service';
import { PreciosController } from './precios.controller';
import { PreciosService } from './precios.service';
import { SeriesController } from './series.controller';
import { SeriesService } from './series.service';

@Module({
  imports: [PgModule],
  controllers: [
    ItemsController,
    TarifasController,
    ComponentesController,
    PreciosController,
    SeriesController,
  ],
  providers: [ItemsService, TarifasService, ComponentesService, PreciosService, SeriesService],
  exports: [ItemsService, TarifasService, ComponentesService, PreciosService, SeriesService],
})
export class ItemsModule {}
