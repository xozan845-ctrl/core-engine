import { Module } from '@nestjs/common';
import { CapacidadesModule } from '../capacidades/capacidades.module';
import { RegistrosController } from './registros.controller';
import { RegistrosService } from './registros.service';

@Module({
  imports: [CapacidadesModule],
  controllers: [RegistrosController],
  providers: [RegistrosService],
  exports: [RegistrosService],
})
export class RegistrosModule {}
