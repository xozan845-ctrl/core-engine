import { Controller, Get, Param, UseGuards } from '@nestjs/common';
import { ApiOkResponse, ApiOperation, ApiParam, ApiSecurity, ApiTags } from '@nestjs/swagger';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { UsuariosService } from '../usuarios/usuarios.service';
import { EmpresasService } from '../empresas/empresas.service';
import { EmpresaResponseDto } from '../empresas/empresas.controller';
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
  constructor(
    private readonly usuarios: UsuariosService,
    private readonly empresas: EmpresasService,
  ) {}

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

  @Get('empresas/:id')
  @ApiOperation({ summary: 'Empresa por id (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id de la empresa' })
  @ApiOkResponse({ type: EmpresaResponseDto, description: 'Empresa encontrada' })
  async empresa(@Param('id') id: string) {
    const empresa = await this.empresas.obtener(id);
    if (!empresa) throw new NotFoundError('Empresa', id);
    return empresa;
  }
}
