import { Module } from '@nestjs/common';
import { ProductosService } from './productos.service';
import { ProductosController } from './productos.controller';
import { ItemsModule } from '../items/items.module';

@Module({
  imports: [ItemsModule],
  providers: [ProductosService],
  exports: [ProductosService],
  controllers: [ProductosController],
})
export class ProductosModule {}
