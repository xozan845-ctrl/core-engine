import { Module } from '@nestjs/common';
import { ProductosModule } from '../productos/productos.module';
import { ServiciosController } from './servicios.controller';
import { ServiciosService } from './servicios.service';

/** Servicios de la empresa (R-PS-02), separados del catálogo de productos. */
@Module({
  imports: [ProductosModule],
  controllers: [ServiciosController],
  providers: [ServiciosService],
  exports: [ServiciosService],
})
export class ServiciosModule {}
