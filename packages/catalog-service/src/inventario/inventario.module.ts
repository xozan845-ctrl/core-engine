import { Module } from '@nestjs/common';
import { InventarioService } from './inventario.service';
import { InventarioController } from './inventario.controller';

@Module({
  providers: [InventarioService],
  exports: [InventarioService],
  controllers: [InventarioController],
})
export class InventarioModule {}