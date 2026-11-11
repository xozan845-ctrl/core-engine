import { Module } from '@nestjs/common';
import { PgModule } from '@core/shared';
import { ItemsController } from './items.controller';
import { ItemsService } from './items.service';
import { TarifasController } from './tarifas.controller';
import { TarifasService } from './tarifas.service';

@Module({
  imports: [PgModule],
  controllers: [ItemsController, TarifasController],
  providers: [ItemsService, TarifasService],
  exports: [ItemsService, TarifasService],
})
export class ItemsModule {}
