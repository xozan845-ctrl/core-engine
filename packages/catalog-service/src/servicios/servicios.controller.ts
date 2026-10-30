import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  Param,
  Patch,
  Post,
  Query,
} from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiNoContentResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiQuery,
  ApiTags,
} from '@nestjs/swagger';
import {
  IsIn,
  IsInt,
  IsOptional,
  IsPositive,
  IsString,
  IsUUID,
  Matches,
  Max,
  Min,
  MinLength,
} from 'class-validator';
import { DomainError, NotFoundError, ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import {
  ActualizarServicioInput,
  CrearServicioInput,
  ServiciosService,
} from './servicios.service';
import { PaginaServiciosResponseDto, ServicioResponseDto } from './servicios.response.dto';

export class CrearServicioRequestDto {
  @ApiProperty({ description: 'Código interno del servicio', example: 'SERV-001' })
  @IsString()
  @Matches(/^[A-Za-z0-9-]{2,32}$/, { message: 'Código inválido (letras, números y guiones, 2-32)' })
  codigo: string;

  @ApiProperty({ description: 'Empresa destino (solo admin/super_admin)', required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  empresa_id?: string;

  @ApiProperty({ description: 'Nombre del servicio', minLength: 2, maxLength: 200 })
  @IsString()
  @MinLength(2)
  nombre: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  categoria?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  marca?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  codigo_barras?: string;

  @ApiProperty({ description: 'Precio decimal', example: '1500.00' })
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio inválido (ej: 1500.00)' })
  precio: string;

  @ApiProperty({ description: 'Costo decimal', required: false, example: '500.00' })
  @IsOptional()
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Costo inválido (ej: 500.00)' })
  costo?: string;

  @ApiProperty({ required: false, example: 'Hora (h)' })
  @IsOptional()
  @IsString()
  unidad?: string;

  @ApiProperty({ description: 'Duración en minutos (1-1440)', minimum: 1, maximum: 1440 })
  @IsInt()
  @Min(1)
  @Max(1440)
  duracion_minutos: number;

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado?: 'activo' | 'inactivo';
}

export class ActualizarServicioRequestDto {
  @ApiProperty({ required: false, minLength: 2 })
  @IsOptional()
  @IsString()
  @MinLength(2)
  nombre?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  categoria?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  marca?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  codigo_barras?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio inválido (ej: 1500.00)' })
  precio?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Costo inválido (ej: 500.00)' })
  costo?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  unidad?: string;

  @ApiProperty({ required: false, minimum: 1, maximum: 1440 })
  @IsOptional()
  @IsInt()
  @Min(1)
  @Max(1440)
  duracion_minutos?: number;

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado?: 'activo' | 'inactivo';
}

export class ListarServiciosRequestDto {
  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  empresa_id?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  q?: string;

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  categoria?: string;

  @ApiProperty({ required: false, minimum: 1 })
  @IsOptional()
  @IsPositive()
  pagina?: number;

  @ApiProperty({ required: false, minimum: 1, maximum: 100 })
  @IsOptional()
  @IsPositive()
  @Max(100)
  limite?: number;
}

@ApiTags('Catálogo · Servicios')
@ApiBearerAuth()
@Controller('api/v1/catalog/servicios')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
export class ServiciosController {
  constructor(private readonly servicios: ServiciosService) {}

  @Get()
  @ApiOperation({ summary: 'Listar servicios de una empresa' })
  @ApiOkResponse({ type: PaginaServiciosResponseDto, description: 'Página de servicios' })
  async listar(
    @Query() query: ListarServiciosRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    const empresaId = this.empresaDeConsulta(usuario, query.empresa_id);
    return this.servicios.listar({ ...query, empresa_id: empresaId });
  }

  @Get(':id')
  @ApiOperation({ summary: 'Detalle de un servicio' })
  @ApiParam({ name: 'id', description: 'Id del servicio' })
  @ApiQuery({ name: 'empresa_id', required: false, format: 'uuid' })
  @ApiOkResponse({ type: ServicioResponseDto, description: 'Servicio encontrado' })
  async detalle(
    @Param('id') id: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    const servicio = await this.servicios.encontrarPorId(
      id,
      this.empresaDeConsulta(usuario, empresaId),
    );
    if (!servicio) throw new NotFoundError('Servicio', id);
    return servicio;
  }

  @Post()
  @ApiOperation({ summary: 'Crear un servicio' })
  @ApiCreatedResponse({ type: ServicioResponseDto, description: 'Servicio creado' })
  async crear(
    @Body() dto: CrearServicioRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    const empresaId = this.empresaDeEscritura(usuario, dto.empresa_id);
    return this.servicios.crear(dto as CrearServicioInput, empresaId);
  }

  @Patch(':id')
  @ApiOperation({ summary: 'Actualizar un servicio' })
  @ApiParam({ name: 'id', description: 'Id del servicio' })
  @ApiQuery({ name: 'empresa_id', required: false, format: 'uuid' })
  @ApiOkResponse({ type: ServicioResponseDto, description: 'Servicio actualizado' })
  async actualizar(
    @Param('id') id: string,
    @Body() dto: ActualizarServicioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.servicios.actualizar(
      id,
      dto as ActualizarServicioInput,
      this.empresaDeConsulta(usuario, empresaId),
    );
  }

  @Delete(':id')
  @HttpCode(204)
  @ApiOperation({ summary: 'Dar de baja un servicio (baja lógica)' })
  @ApiParam({ name: 'id', description: 'Id del servicio' })
  @ApiQuery({ name: 'empresa_id', required: false, format: 'uuid' })
  @ApiNoContentResponse({ description: 'Servicio dado de baja' })
  async eliminar(
    @Param('id') id: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<void> {
    await this.servicios.eliminar(id, this.empresaDeConsulta(usuario, empresaId));
  }

  private empresaDeConsulta(usuario: UsuarioContexto, solicitada?: string): string {
    if (usuario.rol === ROLES.EMPRESARIO) {
      if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
      if (solicitada && solicitada !== usuario.tenant_id) {
        throw new DomainError(
          'EMPRESA_NO_AUTORIZADA',
          'El empresario solo puede consultar los servicios de su empresa.',
        );
      }
      return usuario.tenant_id;
    }
    const empresaId = solicitada ?? usuario.tenant_id;
    if (!empresaId) {
      throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id para operar los servicios.');
    }
    return empresaId;
  }

  private empresaDeEscritura(usuario: UsuarioContexto, solicitada?: string): string {
    if (usuario.rol !== ROLES.SUPER_ADMIN && usuario.rol !== ROLES.ADMIN) {
      if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
      return usuario.tenant_id;
    }
    if (!solicitada) {
      throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id para crear el servicio.');
    }
    return solicitada;
  }
}
