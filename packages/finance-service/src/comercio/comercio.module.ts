import { Module } from '@nestjs/common';
import { ComercioController } from './comercio.controller';
import { ComercioService } from './comercio.service';

/** Operación de comercio (bancos, caja, compras, clientes y códigos). */
@Module({
  controllers: [ComercioController],
  providers: [ComercioService],
  exports: [ComercioService],
})
export class ComercioModule {}
