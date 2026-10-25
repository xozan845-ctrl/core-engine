import { Module } from '@nestjs/common';
import { ObjetosService } from './objetos.service';
import { ObjetosController } from './objetos.controller';
import { PublicStorageController } from './public.controller';
import { BucketsModule } from '../buckets/buckets.module';
import { AlmacenModule } from '../almacen/almacen.module';

@Module({
  imports: [AlmacenModule, BucketsModule],
  providers: [ObjetosService],
  exports: [ObjetosService],
  controllers: [ObjetosController, PublicStorageController],
})
export class ObjetosModule {}
