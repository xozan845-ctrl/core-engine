import { NestFactory } from '@nestjs/core';
import { AppModule } from './app.module';
import { PUERTOS, NOMBRE_SERVICIOS, DtoValidationPipe, DomainErrorFilter, Logger } from '@core/shared';

async function bootstrap(): Promise<void> {
  const app = await NestFactory.create(AppModule);

  app.useGlobalPipes(new DtoValidationPipe());
  app.useGlobalFilters(new DomainErrorFilter());

  app.enableShutdownHooks();
  await app.listen(PUERTOS.INTELLIGENCE);
  Logger.create(NOMBRE_SERVICIOS.INTELLIGENCE).info({
    msg: 'market-intelligence-service listo',
    puerto: PUERTOS.INTELLIGENCE,
  });
}

bootstrap().catch((err: Error) => {
  console.error(JSON.stringify({ nivel: 'error', msg: 'Fallo el arranque', err: err.message }));
  process.exit(1);
});
