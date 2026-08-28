import { Module } from '@nestjs/common';
import { PgModule, MetricsModule } from '@core/shared';
import { FieldModule } from './field/field.module';
import { FieldController } from './field/field.controller';

@Module({
  imports: [PgModule, MetricsModule, FieldModule],
  controllers: [FieldController],
})
export class AppModule {}
