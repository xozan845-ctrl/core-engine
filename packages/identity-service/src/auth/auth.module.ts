import { Module } from '@nestjs/common';
import { UsuariosModule } from '../usuarios/usuarios.module';
import { AuthController } from './auth.controller';
import { AuthService } from './auth.service';
import { SesionesRepository } from './sesiones.repository';

@Module({
  imports: [UsuariosModule],
  controllers: [AuthController],
  providers: [AuthService, SesionesRepository],
  exports: [AuthService],
})
export class AuthModule {}