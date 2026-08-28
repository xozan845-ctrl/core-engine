import { Controller, Get, Param, UseGuards } from '@nestjs/common';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { UsuariosService } from '../usuarios/usuarios.service';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna.
 * El API Gateway nunca los expone al publico.
 */
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(private readonly usuarios: UsuariosService) {}

  @Get('usuarios/:id')
  async usuario(@Param('id') id: string) {
    const usuario = await this.usuarios.encontrarPorId(id);
    if (!usuario) throw new NotFoundError('Usuario', id);
    return usuario;
  }

  @Get('vendedores')
  async vendedores() {
    return this.usuarios.listarVendedores();
  }

  @Get('contar-vendedores')
  async contarVendedores() {
    return { vendedores: await this.usuarios.contarVendedores() };
  }
}