import { Module } from '@nestjs/common';
import { ProductosModule } from '../productos/productos.module';
import { ServiciosModule } from '../servicios/servicios.module';
import { OperacionesController } from './operaciones.controller';
import { OperacionesService } from './operaciones.service';

/** Operaciones avanzadas del catálogo (R-PS-04). */
@Module({
  imports: [ProductosModule, ServiciosModule],
  controllers: [OperacionesController],
  providers: [OperacionesService],
  exports: [OperacionesService],
})
export class OperacionesModule {}
