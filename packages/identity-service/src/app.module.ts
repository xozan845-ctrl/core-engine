import { Module } from '@nestjs/common';
import { PgModule, RabbitModule, OutboxModule, MetricsModule, EmailModule } from '@core/shared';
import { AuthModule } from './auth/auth.module';
import { UsuariosModule } from './usuarios/usuarios.module';
import { SeedService } from './seed/seed.service';
import { InternalController } from './internal/internal.controller';
import { EmpresasModule } from './empresas/empresas.module';
import { ConfiguracionSitioModule } from './configuracion-sitio/configuracion-sitio.module';
import { CapacidadesModule } from './capacidades/capacidades.module';
import { PacksModule } from './packs/packs.module';
import { ExtensionesModule } from './extensiones/extensiones.module';
import { PartesModule } from './partes/partes.module';
import { PersonalModule } from './personal/personal.module';

@Module({
  imports: [PgModule, RabbitModule, OutboxModule, MetricsModule, EmailModule, AuthModule, UsuariosModule, EmpresasModule, ConfiguracionSitioModule, CapacidadesModule, PacksModule, ExtensionesModule, PartesModule, PersonalModule],
  providers: [SeedService],
  controllers: [InternalController],
})
export class AppModule {}
