import { Module } from '@nestjs/common';
import { SchedulingController } from './scheduling.controller';
import { SchedulingConsumer } from './scheduling.consumer';
import { SchedulingService } from './scheduling.service';

@Module({
  controllers: [SchedulingController],
  providers: [SchedulingService, SchedulingConsumer],
  exports: [SchedulingService],
})
export class SchedulingModule {}
