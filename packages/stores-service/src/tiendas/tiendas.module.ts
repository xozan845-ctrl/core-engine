import { Module } from '@nestjs/common';
import { TiendasService } from './tiendas.service';
import { TiendasController, TiendasEmpresaController } from './tiendas.controller';

@Module({
  providers: [TiendasService],
  exports: [TiendasService],
  controllers: [TiendasController, TiendasEmpresaController],
})
export class TiendasModule {}