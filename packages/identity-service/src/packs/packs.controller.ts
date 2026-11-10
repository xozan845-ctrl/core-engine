import { Body, Controller, Get, Param, ParseIntPipe, Post, Query } from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiOkResponse, ApiOperation, ApiParam, ApiProperty, ApiTags } from '@nestjs/swagger';
import { Type } from 'class-transformer';
import {
  IsArray,
  IsBoolean,
  IsInt,
  IsObject,
  IsOptional,
  IsString,
  Length,
  Matches,
  Min,
  ValidateNested,
} from 'class-validator';
import { ROLES, Roles } from '@core/shared';
import { ModeloOperacion, Pack, PacksService, VerticalOperacion } from './packs.service';

class ListarPacksQueryDto {
  @ApiProperty({ required: false, description: 'Código dinámico de la vertical' })
  @IsOptional()
  @IsString()
  @Matches(/^[a-z][a-z0-9_]{1,39}$/)
  vertical_id?: string;
}

class PackCapacidadResponseDto {
  @ApiProperty() capacidad_codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() modulo: string;
  @ApiProperty() activo: boolean;
  @ApiProperty({ type: Object }) config: Record<string, unknown>;
}

class PackModeloResponseDto {
  @ApiProperty() modelo_codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() descripcion: string;
  @ApiProperty() activo: boolean;
  @ApiProperty({ type: Object }) config: Record<string, unknown>;
}

class ModeloOperacionResponseDto implements ModeloOperacion {
  @ApiProperty() codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() descripcion: string;
  @ApiProperty({ type: Object }) config: Record<string, unknown>;
  @ApiProperty() activo: boolean;
}

class VerticalOperacionResponseDto implements VerticalOperacion {
  @ApiProperty() codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() modelo_operacion: string;
  @ApiProperty() activo: boolean;
}

class CrearModeloOperacionRequestDto {
  @ApiProperty({ example: 'servicios_domicilio' })
  @IsString() @Matches(/^[a-z][a-z0-9_]{1,39}$/) codigo: string;
  @ApiProperty({ example: 'Servicios a domicilio' })
  @IsString() @Length(2, 120) nombre: string;
  @ApiProperty({ required: false })
  @IsOptional() @IsString() @Length(0, 500) descripcion?: string;
  @ApiProperty({ required: false, type: Object })
  @IsOptional() @IsObject() config?: Record<string, unknown>;
  @ApiProperty({ required: false, default: true })
  @IsOptional() @IsBoolean() activo?: boolean;
}

class CrearVerticalRequestDto {
  @ApiProperty({ example: 'servicios_domicilio' })
  @IsString() @Matches(/^[a-z][a-z0-9_]{1,39}$/) codigo: string;
  @ApiProperty({ example: 'Servicios a domicilio' })
  @IsString() @Length(2, 120) nombre: string;
  @ApiProperty({ example: 'servicios_agendados' })
  @IsString() @Length(2, 160) modelo_operacion: string;
  @ApiProperty({ required: false, default: true })
  @IsOptional() @IsBoolean() activo?: boolean;
}

class PackBuilderItemRequestDto {
  @ApiProperty({ example: 'agenda_recursos' })
  @IsString() @Matches(/^[a-z][a-z0-9_]{1,59}$/) codigo: string;
  @ApiProperty({ required: false, type: Object })
  @IsOptional() @IsObject() config?: Record<string, unknown>;
  @ApiProperty({ required: false, default: true })
  @IsOptional() @IsBoolean() activo?: boolean;
}

class CrearPackRequestDto {
  @ApiProperty({ example: 'pack_servicios_domicilio' })
  @IsString() @Matches(/^pack_[a-z][a-z0-9_]{1,39}$/) codigo: string;
  @ApiProperty({ minimum: 1, example: 1 })
  @IsInt() @Min(1) version: number;
  @ApiProperty({ example: 'servicios_domicilio' })
  @IsString() @Matches(/^[a-z][a-z0-9_]{1,39}$/) vertical_codigo: string;
  @ApiProperty({ example: 'Servicios a domicilio' })
  @IsString() @Length(2, 120) nombre: string;
  @ApiProperty({ required: false })
  @IsOptional() @IsString() @Length(0, 500) descripcion?: string;
  @ApiProperty({ required: false, type: Object })
  @IsOptional() @IsObject() config?: Record<string, unknown>;
  @ApiProperty({ required: false, type: [PackBuilderItemRequestDto] })
  @IsOptional() @IsArray() @ValidateNested({ each: true }) @Type(() => PackBuilderItemRequestDto)
  modelos?: PackBuilderItemRequestDto[];
  @ApiProperty({ required: false, type: [PackBuilderItemRequestDto] })
  @IsOptional() @IsArray() @ValidateNested({ each: true }) @Type(() => PackBuilderItemRequestDto)
  capacidades?: PackBuilderItemRequestDto[];
}

class PackResponseDto implements Pack {
  @ApiProperty() codigo: string;
  @ApiProperty() version: number;
  @ApiProperty() vertical_codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() descripcion: string;
  @ApiProperty({ type: Object }) config: Record<string, unknown>;
  @ApiProperty({ type: [PackModeloResponseDto] }) modelos: PackModeloResponseDto[];
  @ApiProperty({ type: [PackCapacidadResponseDto] }) capacidades: PackCapacidadResponseDto[];
}

@ApiTags('Packs')
@ApiBearerAuth()
@Controller('api/v2/packs')
export class PacksController {
  constructor(private readonly packs: PacksService) {}

  @Get()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar packs activos disponibles para crear empresas' })
  @ApiOkResponse({ type: [PackResponseDto] })
  listar(@Query() query: ListarPacksQueryDto) {
    return this.packs.listar(query.vertical_id?.trim().toLowerCase());
  }

  @Get('modelos-operacion')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar modelos operativos disponibles para construir packs' })
  @ApiOkResponse({ type: [ModeloOperacionResponseDto] })
  listarModelos() {
    return this.packs.listarModelos();
  }

  @Get('verticales')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar verticales configurables disponibles' })
  @ApiOkResponse({ type: [VerticalOperacionResponseDto] })
  listarVerticales() {
    return this.packs.listarVerticales();
  }

  @Post('modelos-operacion')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'Registrar un modelo operativo reutilizable' })
  @ApiCreatedResponse({ type: ModeloOperacionResponseDto })
  crearModelo(@Body() dto: CrearModeloOperacionRequestDto) {
    return this.packs.crearModelo({
      ...dto,
      descripcion: dto.descripcion ?? '',
      config: dto.config ?? {},
    });
  }

  @Post('verticales')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'Registrar una vertical operativa sin crear un microservicio' })
  @ApiCreatedResponse({ type: VerticalOperacionResponseDto })
  crearVertical(@Body() dto: CrearVerticalRequestDto) {
    return this.packs.crearVertical(dto);
  }

  @Post()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'Crear una plantilla de pack con modelos y capacidades' })
  @ApiCreatedResponse({ type: PackResponseDto })
  crear(@Body() dto: CrearPackRequestDto) {
    return this.packs.crear(dto);
  }

  @Get(':codigo/:version')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Obtener un pack versionado y sus capacidades' })
  @ApiParam({ name: 'codigo', example: 'pack_belleza' })
  @ApiParam({ name: 'version', type: Number, example: 1 })
  @ApiOkResponse({ type: PackResponseDto })
  obtener(@Param('codigo') codigo: string, @Param('version', ParseIntPipe) version: number) {
    return this.packs.obtener(codigo.trim().toLowerCase(), version);
  }
}
