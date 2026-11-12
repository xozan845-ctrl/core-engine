import { Module } from '@nestjs/common';
import { PartesController } from './partes.controller';
import { PartesService } from './partes.service';

@Module({
  controllers: [PartesController],
  providers: [PartesService],
  exports: [PartesService],
})
export class PartesModule {}
