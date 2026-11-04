import { Module } from '@nestjs/common';
import { ConfiguracionSitioController } from './configuracion-sitio.controller';
import { ConfiguracionSitioService } from './configuracion-sitio.service';

@Module({
  controllers: [ConfiguracionSitioController],
  providers: [ConfiguracionSitioService],
  exports: [ConfiguracionSitioService],
})
export class ConfiguracionSitioModule {}
