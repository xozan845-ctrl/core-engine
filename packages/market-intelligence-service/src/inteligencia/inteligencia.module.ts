import { Module } from '@nestjs/common';
import { InteligenciaRepository } from './repositories/inteligencia.repository';
import { InteligenciaService } from './services/inteligencia.service';
import { AgregadorService } from './services/agregador.service';
import { VentasConsumer } from './events/ventas.consumer';
import { MetricasController } from './controllers/metricas.controller';

@Module({
  controllers: [MetricasController],
  providers: [InteligenciaRepository, InteligenciaService, AgregadorService, VentasConsumer],
})
export class InteligenciaModule {}
