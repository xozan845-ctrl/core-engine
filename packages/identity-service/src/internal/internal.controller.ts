import { Controller, Get, Param, UseGuards } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiTags } from '@nestjs/swagger';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { UsuariosService } from '../usuarios/usuarios.service';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna.
 * El API Gateway nunca los expone al publico.
 */
@ApiTags('Usuarios (interno)')
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(private readonly usuarios: UsuariosService) {}

  @Get('usuarios/:id')
  @ApiOperation({ summary: 'Usuario por id (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id del usuario' })
  async usuario(@Param('id') id: string) {
    const usuario = await this.usuarios.encontrarPorId(id);
    if (!usuario) throw new NotFoundError('Usuario', id);
    return usuario;
  }

  @Get('vendedores')
  @ApiOperation({ summary: 'Listar vendedores (servicio→servicio)' })
  async vendedores() {
    return this.usuarios.listarVendedores();
  }

  @Get('contar-vendedores')
  @ApiOperation({ summary: 'Contar vendedores (servicio→servicio)' })
  async contarVendedores() {
    return { vendedores: await this.usuarios.contarVendedores() };
  }
}
