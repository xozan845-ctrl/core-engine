import { Controller, Get, Param, UseGuards } from '@nestjs/common';
import { ApiOkResponse, ApiOperation, ApiParam, ApiSecurity, ApiTags } from '@nestjs/swagger';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { UsuariosService } from '../usuarios/usuarios.service';
import {
  ContarVendedoresResponseDto,
  UsuarioResponseDto,
} from '../usuarios/usuarios.response.dto';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna.
 * El API Gateway nunca los expone al publico.
 */
@ApiTags('Usuarios (interno)')
@ApiSecurity('internalKey')
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(private readonly usuarios: UsuariosService) {}

  @Get('usuarios/:id')
  @ApiOperation({ summary: 'Usuario por id (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id del usuario' })
  @ApiOkResponse({ type: UsuarioResponseDto, description: 'Usuario encontrado' })
  async usuario(@Param('id') id: string) {
    const usuario = await this.usuarios.encontrarPorId(id);
    if (!usuario) throw new NotFoundError('Usuario', id);
    return usuario;
  }

  @Get('vendedores')
  @ApiOperation({ summary: 'Listar vendedores (servicio→servicio)' })
  @ApiOkResponse({ type: [UsuarioResponseDto], description: 'Vendedores registrados' })
  async vendedores() {
    return this.usuarios.listarVendedores();
  }

  @Get('contar-vendedores')
  @ApiOperation({ summary: 'Contar vendedores (servicio→servicio)' })
  @ApiOkResponse({ type: ContarVendedoresResponseDto, description: 'Conteo de vendedores' })
  async contarVendedores() {
    return { vendedores: await this.usuarios.contarVendedores() };
  }
}
