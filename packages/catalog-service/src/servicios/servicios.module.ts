import { Module } from '@nestjs/common';
import { ProductosModule } from '../productos/productos.module';
import { ServiciosService } from './servicios.service';
import { ItemsModule } from '../items/items.module';

/** Servicios de la empresa (R-PS-02) sobre `catalog.items` con tipo servicio. */
@Module({
  imports: [ProductosModule, ItemsModule],
  providers: [ServiciosService],
  exports: [ServiciosService],
})
export class ServiciosModule {}
