import { Module } from '@nestjs/common';
import { LiquidacionesService } from './liquidaciones.service';
import { LiquidacionesController } from './liquidaciones.controller';

@Module({
  providers: [LiquidacionesService],
  exports: [LiquidacionesService],
  controllers: [LiquidacionesController],
})
export class LiquidacionesModule {}