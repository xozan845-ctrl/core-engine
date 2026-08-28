import { Module } from '@nestjs/common';
import { PgModule, RabbitModule, OutboxModule, MetricsModule } from '@core/shared';
import { AuthModule } from './auth/auth.module';
import { UsuariosModule } from './usuarios/usuarios.module';
import { SeedService } from './seed/seed.service';
import { InternalController } from './internal/internal.controller';

@Module({
  imports: [PgModule, RabbitModule, OutboxModule, MetricsModule, AuthModule, UsuariosModule],
  providers: [SeedService],
  controllers: [InternalController],
})
export class AppModule {}