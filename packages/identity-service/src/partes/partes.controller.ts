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
import { IsBoolean, IsIn, IsNumber, IsObject, IsOptional, IsString, IsUUID, Length, Matches, Max, Min } from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import {
  ActualizarDireccionInput,
  ActualizarMedioContactoInput,
  ActualizarParteInput,
  CrearDireccionInput,
  CrearMedioContactoInput,
  CrearParteInput,
  DireccionParte,
  EstadoParte,
  MedioContacto,
  Parte,
  PartesService,
  RelacionParte,
  TipoDireccion,
  TipoMedioContacto,
  TipoParte,
} from './partes.service';

const ROLES_LECTURA = [ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO] as const;
const ROLES_ESCRITURA = [ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO] as const;

export class CrearParteRequestDto implements CrearParteInput {
  @ApiProperty({ enum: ['persona', 'organizacion'] }) @IsIn(['persona', 'organizacion']) tipo: TipoParte;
  @ApiProperty({ minLength: 1, maxLength: 240 }) @IsString() @Length(1, 240) nombre: string;
  @ApiProperty({ required: false, maxLength: 240 }) @IsOptional() @IsString() @Length(0, 240) nombre_comercial?: string;
  @ApiProperty({ required: false, maxLength: 40 }) @IsOptional() @IsString() @Length(0, 40) identificador_tipo?: string;
  @ApiProperty({ required: false, maxLength: 120 }) @IsOptional() @IsString() @Length(0, 120) identificador_valor?: string;
  @ApiProperty({ required: false, enum: ['activo', 'inactivo', 'archivado'], default: 'activo' }) @IsOptional() @IsIn(['activo', 'inactivo', 'archivado']) estado?: EstadoParte;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() metadata?: Record<string, unknown>;
}

export class ActualizarParteRequestDto extends PartialType(CrearParteRequestDto) implements ActualizarParteInput {}

export class CrearMedioContactoRequestDto implements CrearMedioContactoInput {
  @ApiProperty({ enum: ['email', 'telefono', 'whatsapp', 'web', 'otro'] }) @IsIn(['email', 'telefono', 'whatsapp', 'web', 'otro']) tipo: TipoMedioContacto;
  @ApiProperty({ minLength: 1, maxLength: 240 }) @IsString() @Length(1, 240) valor: string;
  @ApiProperty({ required: false, maxLength: 120 }) @IsOptional() @IsString() @Length(0, 120) etiqueta?: string;
  @ApiProperty({ required: false, default: false }) @IsOptional() @IsBoolean() es_principal?: boolean;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() metadata?: Record<string, unknown>;
}

export class ActualizarMedioContactoRequestDto extends PartialType(CrearMedioContactoRequestDto) implements ActualizarMedioContactoInput {}

export class CrearDireccionRequestDto implements CrearDireccionInput {
  @ApiProperty({ enum: ['fiscal', 'comercial', 'entrega', 'operativa', 'otro'] }) @IsIn(['fiscal', 'comercial', 'entrega', 'operativa', 'otro']) tipo: TipoDireccion;
  @ApiProperty({ required: false, maxLength: 120 }) @IsOptional() @IsString() @Length(0, 120) etiqueta?: string;
  @ApiProperty({ minLength: 1, maxLength: 240 }) @IsString() @Length(1, 240) linea_1: string;
  @ApiProperty({ required: false, maxLength: 240 }) @IsOptional() @IsString() @Length(0, 240) linea_2?: string;
  @ApiProperty({ required: false, maxLength: 120 }) @IsOptional() @IsString() @Length(0, 120) ciudad?: string;
  @ApiProperty({ required: false, maxLength: 120 }) @IsOptional() @IsString() @Length(0, 120) departamento?: string;
  @ApiProperty({ required: false, maxLength: 120 }) @IsOptional() @IsString() @Length(0, 120) pais?: string;
  @ApiProperty({ required: false, maxLength: 32 }) @IsOptional() @IsString() @Length(0, 32) codigo_postal?: string;
  @ApiProperty({ required: false, maxLength: 500 }) @IsOptional() @IsString() @Length(0, 500) referencias?: string;
  @ApiProperty({ required: false, nullable: true, minimum: -90, maximum: 90 }) @IsOptional() @IsNumber() @Min(-90) @Max(90) latitud?: number | null;
  @ApiProperty({ required: false, nullable: true, minimum: -180, maximum: 180 }) @IsOptional() @IsNumber() @Min(-180) @Max(180) longitud?: number | null;
  @ApiProperty({ required: false, default: false }) @IsOptional() @IsBoolean() es_principal?: boolean;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() metadata?: Record<string, unknown>;
}

export class ActualizarDireccionRequestDto extends PartialType(CrearDireccionRequestDto) implements ActualizarDireccionInput {}

export class CrearRelacionRequestDto {
  @ApiProperty({ format: 'uuid' }) @IsUUID() parte_destino_id: string;
  @ApiProperty({ example: 'cliente', pattern: '^[a-z][a-z0-9_]{1,39}$' }) @IsString() @Matches(/^[a-z][a-z0-9_]{1,39}$/) tipo: string;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() metadata?: Record<string, unknown>;
}

class ParteResponseDto implements Parte {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ enum: ['persona', 'organizacion'] }) tipo: TipoParte;
  @ApiProperty() nombre: string;
  @ApiProperty() nombre_comercial: string;
  @ApiProperty() identificador_tipo: string;
  @ApiProperty() identificador_valor: string;
  @ApiProperty({ enum: ['activo', 'inactivo', 'archivado'] }) estado: EstadoParte;
  @ApiProperty({ type: Object, additionalProperties: true }) metadata: Record<string, unknown>;
  @ApiProperty({ format: 'uuid' }) creado_por: string;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
  @ApiProperty({ nullable: true, format: 'date-time' }) eliminado_en: string | null;
}

class MedioContactoResponseDto implements MedioContacto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) parte_id: string;
  @ApiProperty({ enum: ['email', 'telefono', 'whatsapp', 'web', 'otro'] }) tipo: TipoMedioContacto;
  @ApiProperty() valor: string;
  @ApiProperty() etiqueta: string;
  @ApiProperty() es_principal: boolean;
  @ApiProperty({ nullable: true, format: 'date-time' }) verificado_en: string | null;
  @ApiProperty({ type: Object, additionalProperties: true }) metadata: Record<string, unknown>;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
  @ApiProperty({ nullable: true, format: 'date-time' }) eliminado_en: string | null;
}

class DireccionResponseDto implements DireccionParte {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) parte_id: string;
  @ApiProperty({ enum: ['fiscal', 'comercial', 'entrega', 'operativa', 'otro'] }) tipo: TipoDireccion;
  @ApiProperty() etiqueta: string;
  @ApiProperty() linea_1: string;
  @ApiProperty() linea_2: string;
  @ApiProperty() ciudad: string;
  @ApiProperty() departamento: string;
  @ApiProperty() pais: string;
  @ApiProperty() codigo_postal: string;
  @ApiProperty() referencias: string;
  @ApiProperty({ nullable: true }) latitud: number | null;
  @ApiProperty({ nullable: true }) longitud: number | null;
  @ApiProperty() es_principal: boolean;
  @ApiProperty({ type: Object, additionalProperties: true }) metadata: Record<string, unknown>;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
  @ApiProperty({ nullable: true, format: 'date-time' }) eliminado_en: string | null;
}

class RelacionResponseDto implements RelacionParte {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) parte_origen_id: string;
  @ApiProperty({ format: 'uuid' }) parte_destino_id: string;
  @ApiProperty() tipo: string;
  @ApiProperty({ type: Object, additionalProperties: true }) metadata: Record<string, unknown>;
  @ApiProperty() activo: boolean;
  @ApiProperty({ format: 'uuid' }) creado_por: string;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
  @ApiProperty({ required: false, type: Object }) parte_relacionada?: { id: string; nombre: string; tipo: TipoParte };
}

@ApiTags('Partes universales')
@ApiBearerAuth()
@Controller('api/v2/empresas/:empresaId/partes')
export class PartesController {
  constructor(private readonly partes: PartesService) {}

  @Get()
  @Roles(...ROLES_LECTURA)
  @ApiOperation({ summary: 'Listar personas y organizaciones de una empresa' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiQuery({ name: 'q', required: false })
  @ApiQuery({ name: 'tipo', required: false, enum: ['persona', 'organizacion'] })
  @ApiQuery({ name: 'limite', required: false, minimum: 1, maximum: 100, type: Number })
  @ApiOkResponse({ type: [ParteResponseDto] })
  listar(@Param('empresaId') empresaId: string, @Query('q') q: string | undefined, @Query('tipo') tipo: TipoParte | undefined, @Query('limite') limite: string | undefined, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.listar(empresaId, { q, tipo, limite: limite ? Number(limite) : undefined }, actor);
  }

  @Post()
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Crear una persona u organización universal' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiCreatedResponse({ type: ParteResponseDto })
  crear(@Param('empresaId') empresaId: string, @Body() dto: CrearParteRequestDto, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.crear(empresaId, dto, actor);
  }

  @Get(':parteId')
  @Roles(...ROLES_LECTURA)
  @ApiOperation({ summary: 'Obtener una parte universal' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiParam({ name: 'parteId', format: 'uuid' })
  @ApiOkResponse({ type: ParteResponseDto })
  obtener(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.obtener(empresaId, parteId, actor);
  }

  @Patch(':parteId')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Actualizar una parte universal' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiParam({ name: 'parteId', format: 'uuid' })
  @ApiOkResponse({ type: ParteResponseDto })
  actualizar(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @Body() dto: ActualizarParteRequestDto, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.actualizar(empresaId, parteId, dto, actor);
  }

  @Delete(':parteId')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Archivar una parte universal y sus datos derivados' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiParam({ name: 'parteId', format: 'uuid' })
  @ApiOkResponse({ description: 'Parte archivada' })
  eliminar(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.eliminar(empresaId, parteId, actor);
  }

  @Get(':parteId/contactos')
  @Roles(...ROLES_LECTURA)
  @ApiOperation({ summary: 'Listar medios de contacto de una parte' })
  @ApiOkResponse({ type: [MedioContactoResponseDto] })
  listarContactos(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.listarContactos(empresaId, parteId, actor);
  }

  @Post(':parteId/contactos')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Agregar un medio de contacto' })
  @ApiCreatedResponse({ type: MedioContactoResponseDto })
  crearContacto(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @Body() dto: CrearMedioContactoRequestDto, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.crearContacto(empresaId, parteId, dto, actor);
  }

  @Patch(':parteId/contactos/:contactoId')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Actualizar un medio de contacto' })
  @ApiOkResponse({ type: MedioContactoResponseDto })
  actualizarContacto(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @Param('contactoId') contactoId: string, @Body() dto: ActualizarMedioContactoRequestDto, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.actualizarContacto(empresaId, parteId, contactoId, dto, actor);
  }

  @Delete(':parteId/contactos/:contactoId')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Archivar un medio de contacto' })
  @ApiOkResponse({ description: 'Medio de contacto archivado' })
  eliminarContacto(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @Param('contactoId') contactoId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.eliminarContacto(empresaId, parteId, contactoId, actor);
  }

  @Get(':parteId/direcciones')
  @Roles(...ROLES_LECTURA)
  @ApiOperation({ summary: 'Listar direcciones de una parte' })
  @ApiOkResponse({ type: [DireccionResponseDto] })
  listarDirecciones(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.listarDirecciones(empresaId, parteId, actor);
  }

  @Post(':parteId/direcciones')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Agregar una dirección' })
  @ApiCreatedResponse({ type: DireccionResponseDto })
  crearDireccion(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @Body() dto: CrearDireccionRequestDto, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.crearDireccion(empresaId, parteId, dto, actor);
  }

  @Patch(':parteId/direcciones/:direccionId')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Actualizar una dirección' })
  @ApiOkResponse({ type: DireccionResponseDto })
  actualizarDireccion(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @Param('direccionId') direccionId: string, @Body() dto: ActualizarDireccionRequestDto, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.actualizarDireccion(empresaId, parteId, direccionId, dto, actor);
  }

  @Delete(':parteId/direcciones/:direccionId')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Archivar una dirección' })
  @ApiOkResponse({ description: 'Dirección archivada' })
  eliminarDireccion(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @Param('direccionId') direccionId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.eliminarDireccion(empresaId, parteId, direccionId, actor);
  }

  @Get(':parteId/relaciones')
  @Roles(...ROLES_LECTURA)
  @ApiOperation({ summary: 'Listar relaciones de una parte' })
  @ApiOkResponse({ type: [RelacionResponseDto] })
  listarRelaciones(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.listarRelaciones(empresaId, parteId, actor);
  }

  @Post(':parteId/relaciones')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Relacionar dos partes de la misma empresa' })
  @ApiCreatedResponse({ type: RelacionResponseDto })
  crearRelacion(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @Body() dto: CrearRelacionRequestDto, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.crearRelacion(empresaId, parteId, dto.parte_destino_id, dto.tipo, dto.metadata, actor);
  }

  @Delete(':parteId/relaciones/:relacionId')
  @Roles(...ROLES_ESCRITURA)
  @ApiOperation({ summary: 'Desactivar una relación' })
  @ApiOkResponse({ description: 'Relación desactivada' })
  eliminarRelacion(@Param('empresaId') empresaId: string, @Param('parteId') parteId: string, @Param('relacionId') relacionId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.partes.eliminarRelacion(empresaId, parteId, relacionId, actor);
  }
}
