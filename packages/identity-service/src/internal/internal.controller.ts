import { Body, Controller, Get, Param, Post, Query, UseGuards } from '@nestjs/common';
import { ApiOkResponse, ApiOperation, ApiParam, ApiProperty, ApiSecurity, ApiTags } from '@nestjs/swagger';
import { ArrayNotEmpty, IsArray, IsString, IsUUID } from 'class-validator';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { UsuariosService } from '../usuarios/usuarios.service';
import { EmpresasService } from '../empresas/empresas.service';
import { EmpresaResponseDto } from '../empresas/empresas.controller';
import { ConfiguracionSitioService } from '../configuracion-sitio/configuracion-sitio.service';
import { TemaOverridesResponseDto } from '../personalizacion/tema.dto';
import { CapacidadesService } from '../capacidades/capacidades.service';
import { PartesService } from '../partes/partes.service';
import {
  ContarVendedoresResponseDto,
  UsuarioResponseDto,
} from '../usuarios/usuarios.response.dto';

export class VerificarCapacidadesRequestDto {
  @ApiProperty({ type: [String], format: 'uuid' })
  @IsArray()
  @ArrayNotEmpty()
  @IsUUID('4', { each: true })
  empresa_ids: string[];

  @ApiProperty({ type: [String] })
  @IsArray()
  @ArrayNotEmpty()
  @IsString({ each: true })
  capacidades: string[];
}

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
    private readonly configuracion: ConfiguracionSitioService,
    private readonly capacidades: CapacidadesService,
    private readonly partes: PartesService,
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

  @Get('empresas/:id/tema')
  @ApiOperation({ summary: 'Overrides de tema de una empresa (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id de la empresa' })
  @ApiOkResponse({
    type: TemaOverridesResponseDto,
    description: 'Overrides de plantilla y tema de la empresa',
  })
  async empresaTema(@Param('id') id: string) {
    return this.empresas.obtenerTemaOverrides(id);
  }

  @Get('configuracion-sitio/tema')
  @ApiOperation({ summary: 'Overrides de tema de la plataforma (servicio→servicio)' })
  @ApiOkResponse({
    type: TemaOverridesResponseDto,
    description: 'Overrides de plantilla y tema de la plataforma',
  })
  async plataformaTema() {
    return this.configuracion.obtenerTemaOverrides();
  }

  @Post('empresas/capacidades')
  @ApiOperation({ summary: 'Capacidades activas por empresa (servicio→servicio)' })
  @ApiOkResponse({ description: 'Capacidades encontradas' })
  async capacidadesDeEmpresas(@Body() dto: VerificarCapacidadesRequestDto) {
    const filas = await this.capacidades.activasDeEmpresas(dto.empresa_ids, dto.capacidades);
    return { capacidades: filas };
  }

  @Get('partes/:id')
  @ApiOperation({ summary: 'Parte por id y empresa (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id de la parte' })
  @ApiOkResponse({ description: 'Parte encontrada' })
  async parte(@Param('id') id: string, @Query('empresa_id') empresaId?: string) {
    if (!empresaId) throw new NotFoundError('Parte', id);
    const pertenece = await this.partes.perteneceAEmpresa(empresaId, id);
    if (!pertenece) throw new NotFoundError('Parte', id);
    return { id, empresa_id: empresaId };
  }
}
