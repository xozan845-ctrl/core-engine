import { Module } from '@nestjs/common';
import { ProductosService } from './productos.service';
import { ProductosController } from './productos.controller';

@Module({
  providers: [ProductosService],
  exports: [ProductosService],
  controllers: [ProductosController],
})
export class ProductosModule {}