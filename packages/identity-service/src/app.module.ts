import { Module } from '@nestjs/common';
import { PgModule, RabbitModule, OutboxModule, MetricsModule } from '@core/shared';
import { AuthModule } from './auth/auth.module';
import { UsuariosModule } from './usuarios/usuarios.module';
import { SeedService } from './seed/seed.service';
import { InternalController } from './internal/internal.controller';
import { EmpresasModule } from './empresas/empresas.module';
import { InvitacionesModule } from './invitaciones/invitaciones.module';

@Module({
  imports: [PgModule, RabbitModule, OutboxModule, MetricsModule, AuthModule, UsuariosModule, EmpresasModule, InvitacionesModule],
  providers: [SeedService],
  controllers: [InternalController],
})
export class AppModule {}
