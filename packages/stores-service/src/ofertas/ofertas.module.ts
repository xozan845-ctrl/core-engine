import { Module } from '@nestjs/common';
import { TiendasModule } from '../tiendas/tiendas.module';
import { OfertasService } from './ofertas.service';
import { OfertasController } from './ofertas.controller';

@Module({
  imports: [TiendasModule],
  providers: [OfertasService],
  exports: [OfertasService],
  controllers: [OfertasController],
})
export class OfertasModule {}