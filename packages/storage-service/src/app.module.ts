import { Module } from '@nestjs/common';
import { MetricsModule, OutboxModule, PgModule, RabbitModule } from '@core/shared';
import { AlmacenModule } from './almacen/almacen.module';
import { BucketsModule } from './buckets/buckets.module';
import { ObjetosModule } from './objetos/objetos.module';
import { AdminStorageController } from './admin/admin.controller';
import { InternalStorageController } from './internal/internal.controller';

@Module({
  imports: [
    PgModule,
    RabbitModule,
    OutboxModule,
    MetricsModule,
    AlmacenModule,
    BucketsModule,
    ObjetosModule,
  ],
  controllers: [AdminStorageController, InternalStorageController],
})
export class AppModule {}
