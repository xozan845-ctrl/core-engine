import { Module } from '@nestjs/common';
import { FieldService } from './field.service';
import { FieldController } from './field.controller';

@Module({
  providers: [FieldService],
  exports: [FieldService],
})
export class FieldModule {}
