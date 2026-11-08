import { Body, Controller, Get, HttpCode, HttpStatus, Param, Patch, Post } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiTags,
} from '@nestjs/swagger';
import { IsBoolean, IsObject, IsOptional, IsString, Length, Matches } from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { EmpresaExtension, ExtensionesService } from './extensiones.service';

export class CrearExtensionRequestDto {
  @ApiProperty({ example: 'ficha_cliente' }) @IsString() @Length(2, 40) @Matches(/^[a-z][a-z0-9_]{1,39}$/) codigo: string;
  @ApiProperty({ example: 'Ficha de cliente' }) @IsString() @Length(2, 120) nombre: string;
  @ApiProperty({ example: 'cliente' }) @IsString() @Length(2, 40) @Matches(/^[a-z][a-z0-9_]{1,39}$/) recurso: string;
  @ApiProperty({ required: false, maxLength: 1000 }) @IsOptional() @IsString() @Length(0, 1000) descripcion?: string;
  @ApiProperty({ type: Object, additionalProperties: true }) @IsObject() esquema: Record<string, unknown>;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() formulario?: Record<string, unknown>;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() reglas?: Record<string, unknown>;
  @ApiProperty({ required: false, default: true }) @IsOptional() @IsBoolean() activo?: boolean;
}

export class ActualizarExtensionRequestDto {
  @ApiProperty() @IsBoolean() activo: boolean;
}

export class ValidarExtensionRequestDto {
  @ApiProperty({ type: Object, additionalProperties: true }) @IsObject() datos: Record<string, unknown>;
}

class ExtensionResponseDto implements EmpresaExtension {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty() codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() recurso: string;
  @ApiProperty() descripcion: string;
  @ApiProperty({ type: Object, additionalProperties: true }) esquema: Record<string, unknown>;
  @ApiProperty({ type: Object, additionalProperties: true }) formulario: Record<string, unknown>;
  @ApiProperty({ type: Object, additionalProperties: true }) reglas: Record<string, unknown>;
  @ApiProperty() version: number;
  @ApiProperty() activo: boolean;
  @ApiProperty({ format: 'uuid' }) creado_por: string;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

class ValidacionExtensionResponseDto {
  @ApiProperty() valido: boolean;
  @ApiProperty({ type: [Object] }) errores: Array<Record<string, unknown>>;
}

@ApiTags('Extensiones configurables')
@ApiBearerAuth()
@Controller('api/v1/empresas/:empresaId/extensiones')
export class ExtensionesController {
  constructor(private readonly extensiones: ExtensionesService) {}

  @Get()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  @ApiOperation({ summary: 'Listar extensiones configurables de una empresa' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiOkResponse({ type: [ExtensionResponseDto] })
  listar(@Param('empresaId') empresaId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.extensiones.listar(empresaId, actor);
  }

  @Post()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Crear una extensión o una nueva versión' })
  @ApiCreatedResponse({ type: ExtensionResponseDto })
  crear(@Param('empresaId') empresaId: string, @Body() dto: CrearExtensionRequestDto, @UsuarioActual() actor: UsuarioContexto) {
    return this.extensiones.crear(empresaId, dto, actor);
  }

  @Patch(':codigo')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Activar o desactivar la versión vigente de una extensión' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiParam({ name: 'codigo' })
  @ApiOkResponse({ type: ExtensionResponseDto })
  cambiarEstado(
    @Param('empresaId') empresaId: string,
    @Param('codigo') codigo: string,
    @Body() dto: ActualizarExtensionRequestDto,
    @UsuarioActual() actor: UsuarioContexto,
  ) {
    return this.extensiones.cambiarEstado(empresaId, codigo, dto.activo, actor);
  }

  @Post(':codigo/validar')
  @HttpCode(HttpStatus.OK)
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  @ApiOperation({ summary: 'Validar datos contra la extensión activa' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiParam({ name: 'codigo' })
  @ApiOkResponse({ type: ValidacionExtensionResponseDto })
  validar(
    @Param('empresaId') empresaId: string,
    @Param('codigo') codigo: string,
    @Body() dto: ValidarExtensionRequestDto,
    @UsuarioActual() actor: UsuarioContexto,
  ) {
    return this.extensiones.validar(empresaId, codigo, dto.datos, actor);
  }
}
