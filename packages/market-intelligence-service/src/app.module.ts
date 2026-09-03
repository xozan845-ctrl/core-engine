import { Module } from '@nestjs/common';
import { APP_INTERCEPTOR } from '@nestjs/core';
import {
  PgModule,
  RabbitModule,
  OutboxModule,
  MetricsModule,
  TrazabilidadInterceptor,
} from '@core/shared';
import { InteligenciaModule } from './inteligencia/inteligencia.module';

@Module({
  imports: [
    PgModule,
    RabbitModule,
    OutboxModule,
    MetricsModule,
    InteligenciaModule,
  ],
  providers: [
    { provide: APP_INTERCEPTOR, useClass: TrazabilidadInterceptor },
  ],
})
export class AppModule {}

