import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import {
  InvitacionesEmpresaController,
  InvitacionesPublicasController,
} from './invitaciones.controller';
import { InvitacionesService } from './invitaciones.service';

@Module({
  imports: [AuthModule],
  controllers: [InvitacionesEmpresaController, InvitacionesPublicasController],
  providers: [InvitacionesService],
  exports: [InvitacionesService],
})
export class InvitacionesModule {}
