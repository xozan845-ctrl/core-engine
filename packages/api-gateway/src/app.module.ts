import { Module, MiddlewareConsumer, NestModule, RequestMethod } from '@nestjs/common';
import { APP_INTERCEPTOR } from '@nestjs/core';
import {
  PgModule,
  RabbitModule,
  OutboxModule,
  MetricsModule,
  TrazabilidadInterceptor,
} from '@core/shared';
import { ProxyService } from './proxy/proxy.service';
import { PasarelaMiddleware } from './proxy/pasarela.middleware';
import { RateLimitMiddleware } from './proxy/rate-limit.middleware';

@Module({
  imports: [
    PgModule,
    RabbitModule,
    OutboxModule,
    MetricsModule,
  ],
  providers: [
    ProxyService,
    RateLimitMiddleware,
    { provide: APP_INTERCEPTOR, useClass: TrazabilidadInterceptor },
  ],
})
export class AppModule implements NestModule {
  configure(consumer: MiddlewareConsumer): void {
    consumer
      .apply(RateLimitMiddleware, PasarelaMiddleware)
      .forRoutes({ path: '*', method: RequestMethod.ALL });
  }
}