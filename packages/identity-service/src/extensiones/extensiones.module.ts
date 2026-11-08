import { Module } from '@nestjs/common';
import { ExtensionesController } from './extensiones.controller';
import { ExtensionesService } from './extensiones.service';

@Module({
  controllers: [ExtensionesController],
  providers: [ExtensionesService],
  exports: [ExtensionesService],
})
export class ExtensionesModule {}
