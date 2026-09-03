import { Module } from '@nestjs/common';
import { ScheduleModule } from '@nestjs/schedule';
import { InteligenciaRepository } from './repositories/inteligencia.repository';
import { InteligenciaService } from './services/inteligencia.service';
import { AgregadorService } from './services/agregador.service';
import { VentasConsumer } from './events/ventas.consumer';
import { MetricasController } from './controllers/metricas.controller';
import { PrediccionController } from './controllers/prediccion.controller';
import { DataQualityService } from './services/data-quality.service';
import { ViewRefresherService } from './services/view-refresher.service';
import { FeatureComputerService } from './services/feature-computer.service';

@Module({
  imports: [ScheduleModule.forRoot()],
  controllers: [MetricasController, PrediccionController],
  providers: [
    InteligenciaRepository,
    InteligenciaService,
    AgregadorService,
    VentasConsumer,
    DataQualityService,
    ViewRefresherService,
    FeatureComputerService,
  ],
})
export class InteligenciaModule {}

