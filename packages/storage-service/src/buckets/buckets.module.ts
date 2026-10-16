import { Module } from '@nestjs/common';
import { BucketsService } from './buckets.service';
import { BucketsController } from './buckets.controller';
import { AlmacenModule } from '../almacen/almacen.module';

@Module({
  imports: [AlmacenModule],
  providers: [BucketsService],
  exports: [BucketsService],
  controllers: [BucketsController],
})
export class BucketsModule {}
