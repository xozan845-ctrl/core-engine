import { Body, Controller, Delete, Get, Param, Patch, Post, Query } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiQuery,
  ApiTags,
  PartialType,
} from '@nestjs/swagger';
import {
  ArrayMaxSize,
  IsArray,
  IsObject,
  IsOptional,
  IsString,
  IsUUID,
  Length,
  Matches,
} from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { RegistroParte, RegistrosService } from './registros.service';

const ROLES_ACCESO = [
  ROLES.SUPER_ADMIN,
  ROLES.ADMIN,
  ROLES.EMPRESARIO,
  ROLES.VENDEDOR,
  ROLES.COORDINADOR,
  ROLES.SUPERVISOR,
  ROLES.OPERATIVO,
] as const;

export class CrearRegistroRequestDto {
  @ApiProperty({
    description: 'Tipo de registro (historia_clinica, receta, nota, inspeccion, expediente...)',
    example: 'historia_clinica',
    pattern: '^[a-z][a-z0-9_]{1,39}$',
  })
  @IsString()
  @Matches(/^[a-z][a-z0-9_]{1,39}$/)
  tipo: string;

  @ApiProperty({ required: false, maxLength: 200 })
  @IsOptional()
  @IsString()
  @Length(0, 200)
  titulo?: string;

  @ApiProperty({ required: false, type: Object, additionalProperties: true })
  @IsOptional()
  @IsObject()
  datos?: Record<string, unknown>;

  @ApiProperty({
    required: false,
    type: [String],
    format: 'uuid',
    description: 'Objetos de storage ya confirmados (maximo 10)',
  })
  @IsOptional()
  @IsArray()
  @ArrayMaxSize(10)
  @IsUUID('all', { each: true })
  adjuntos?: string[];

  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  cita_id?: string;

  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  item_id?: string;
}

export class ActualizarRegistroRequestDto extends PartialType(CrearRegistroRequestDto) {}

export class RegistroParteResponseDto implements RegistroParte {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) parte_id: string;
  @ApiProperty({ example: 'historia_clinica' }) tipo: string;
  @ApiProperty() titulo: string;
  @ApiProperty({ type: Object, additionalProperties: true }) datos: Record<string, unknown>;
  @ApiProperty({ type: [String], format: 'uuid' }) adjuntos: string[];
  @ApiProperty({ nullable: true, format: 'uuid' }) cita_id: string | null;
  @ApiProperty({ nullable: true, format: 'uuid' }) item_id: string | null;
  @ApiProperty({ format: 'uuid' }) autor_id: string;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

@ApiTags('Partes · Registros')
@ApiBearerAuth()
@Controller('api/v2/empresas/:empresaId/partes/:parteId/registros')
@Roles(...ROLES_ACCESO)
export class RegistrosController {
  constructor(private readonly registros: RegistrosService) {}

  @Get()
  @ApiOperation({ summary: 'Listar registros de una parte (historia, recetas, expedientes)' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiParam({ name: 'parteId', format: 'uuid' })
  @ApiQuery({ name: 'tipo', required: false })
  @ApiQuery({ name: 'limite', required: false, type: Number })
  @ApiOkResponse({ type: [RegistroParteResponseDto], description: 'Registros de la parte' })
  listar(
    @Param('empresaId') empresaId: string,
    @Param('parteId') parteId: string,
    @Query('tipo') tipo: string | undefined,
    @Query('limite') limite: string | undefined,
    @UsuarioActual() actor: UsuarioContexto,
  ): Promise<RegistroParte[]> {
    return this.registros.listar(empresaId, parteId, actor, tipo, limite ? Number(limite) : undefined);
  }

  @Post()
  @ApiOperation({ summary: 'Crear un registro para una parte' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiParam({ name: 'parteId', format: 'uuid' })
  @ApiCreatedResponse({ type: RegistroParteResponseDto, description: 'Registro creado' })
  crear(
    @Param('empresaId') empresaId: string,
    @Param('parteId') parteId: string,
    @Body() dto: CrearRegistroRequestDto,
    @UsuarioActual() actor: UsuarioContexto,
  ): Promise<RegistroParte> {
    return this.registros.crear(empresaId, parteId, dto, actor);
  }

  @Get(':registroId')
  @ApiOperation({ summary: 'Consultar un registro de la parte' })
  @ApiParam({ name: 'registroId', format: 'uuid' })
  @ApiOkResponse({ type: RegistroParteResponseDto, description: 'Registro encontrado' })
  obtener(
    @Param('empresaId') empresaId: string,
    @Param('parteId') parteId: string,
    @Param('registroId') registroId: string,
    @UsuarioActual() actor: UsuarioContexto,
  ): Promise<RegistroParte> {
    return this.registros.obtener(empresaId, parteId, registroId, actor);
  }

  @Patch(':registroId')
  @ApiOperation({ summary: 'Actualizar un registro de la parte' })
  @ApiParam({ name: 'registroId', format: 'uuid' })
  @ApiOkResponse({ type: RegistroParteResponseDto, description: 'Registro actualizado' })
  actualizar(
    @Param('empresaId') empresaId: string,
    @Param('parteId') parteId: string,
    @Param('registroId') registroId: string,
    @Body() dto: ActualizarRegistroRequestDto,
    @UsuarioActual() actor: UsuarioContexto,
  ): Promise<RegistroParte> {
    return this.registros.actualizar(empresaId, parteId, registroId, dto, actor);
  }

  @Delete(':registroId')
  @ApiOperation({ summary: 'Archivar (borrado logico) un registro de la parte' })
  @ApiParam({ name: 'registroId', format: 'uuid' })
  @ApiOkResponse({ description: 'Registro archivado' })
  eliminar(
    @Param('empresaId') empresaId: string,
    @Param('parteId') parteId: string,
    @Param('registroId') registroId: string,
    @UsuarioActual() actor: UsuarioContexto,
  ): Promise<{ ok: true }> {
    return this.registros.eliminar(empresaId, parteId, registroId, actor);
  }
}
