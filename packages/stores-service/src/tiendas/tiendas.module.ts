import { Module } from '@nestjs/common';
import { TiendasService } from './tiendas.service';
import { TiendasController } from './tiendas.controller';

@Module({
  providers: [TiendasService],
  exports: [TiendasService],
  controllers: [TiendasController],
})
export class TiendasModule {}