import { NestFactory } from '@nestjs/core';
import { AppModule } from './app.module';
import { PUERTOS, DtoValidationPipe, DomainErrorFilter } from '@core/shared';

async function bootstrap() {
  const app = await NestFactory.create(AppModule);

  app.useGlobalPipes(new DtoValidationPipe());
  app.useGlobalFilters(new DomainErrorFilter());

  await app.listen(PUERTOS.INTELLIGENCE);
  console.log(`market-intelligence-service escuchando en puerto ${PUERTOS.INTELLIGENCE}`);
}
bootstrap();
