import { Module } from '@nestjs/common';
import { TiendasService } from './tiendas.service';
import {
  TiendasController,
  TiendasEmpresaController,
  TiendasSlugController,
} from './tiendas.controller';

@Module({
  providers: [TiendasService],
  exports: [TiendasService],
  controllers: [TiendasController, TiendasEmpresaController, TiendasSlugController],
})
export class TiendasModule {}