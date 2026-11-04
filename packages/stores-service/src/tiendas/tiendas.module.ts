import { Module } from '@nestjs/common';
import { TiendasService } from './tiendas.service';
import { TemaHerenciaService } from './herencia-tema';
import {
  TiendasController,
  TiendasEmpresaController,
  TiendasSlugController,
} from './tiendas.controller';

@Module({
  providers: [TiendasService, TemaHerenciaService],
  exports: [TiendasService],
  controllers: [TiendasController, TiendasEmpresaController, TiendasSlugController],
})
export class TiendasModule {}