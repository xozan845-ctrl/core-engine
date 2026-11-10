import { Module } from '@nestjs/common';
import { ProductosService } from './productos.service';
import { ItemsModule } from '../items/items.module';

@Module({
  imports: [ItemsModule],
  providers: [ProductosService],
  exports: [ProductosService],
})
export class ProductosModule {}
