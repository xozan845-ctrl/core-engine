import { Module } from '@nestjs/common';
import { SchedulingController } from './scheduling.controller';
import { SchedulingConsumer } from './scheduling.consumer';
import { SchedulingService } from './scheduling.service';
import { InternalSchedulingController } from './internal-scheduling.controller';

@Module({
  controllers: [SchedulingController, InternalSchedulingController],
  providers: [SchedulingService, SchedulingConsumer],
  exports: [SchedulingService],
})
export class SchedulingModule {}
