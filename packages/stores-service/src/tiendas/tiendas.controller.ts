import { Body, Controller, Get, Param, Patch, Post } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiResponse,
  ApiTags,
} from '@nestjs/swagger';
import { IsBoolean, IsIn, IsInt, IsOptional, IsString, IsUUID, Length, Max, Min } from 'class-validator';
import { TiendasService, Tienda } from './tiendas.service';
import { TiendaResponseDto } from './tiendas.response.dto';
import { BACKGROUND_POSICIONES, BackgroundInput, BackgroundPosition } from './media-background';
import { Roles, ROLES, UsuarioActual, UsuarioContexto, NotFoundError } from '@core/shared';

export class CrearTiendaRequestDto {
  @ApiProperty({ description: 'Nombre de la tienda', minLength: 2, maxLength: 100, example: 'Mi tienda' })
  @IsString()
  @Length(2, 100)
  nombre: string;

  @ApiProperty({ description: 'Descripción de la tienda', required: false, example: 'Ropa y accesorios' })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({
    description: 'Slug público. Si se omite se deriva del nombre y se resuelven colisiones con sufijos.',
    required: false,
    minLength: 2,
    maxLength: 80,
    example: 'tienda-de-la-vendedora-piloto',
  })
  @IsOptional()
  @IsString()
  @Length(2, 80)
  slug?: string;
}

export class ActualizarLogoTiendaRequestDto {
  @ApiProperty({
    description: 'Objeto de storage público confirmado como imagen; null para quitar el logo',
    required: false,
    nullable: true,
    format: 'uuid',
  })
  @IsOptional()
  @IsUUID()
  logo_objeto_id?: string | null;
}

export class ActualizarBackgroundTiendaRequestDto implements BackgroundInput {
  @ApiProperty({
    description: 'Objeto de storage del fondo; null limpia el bloque completo',
    required: false,
    nullable: true,
    type: String,
    format: 'uuid',
  })
  @IsOptional()
  @IsUUID()
  background_objeto_id?: string | null;

  @ApiProperty({
    description: 'Imagen de portada para un fondo de video',
    required: false,
    nullable: true,
    type: String,
    format: 'uuid',
  })
  @IsOptional()
  @IsUUID()
  background_poster_objeto_id?: string | null;

  @ApiProperty({ required: false, maxLength: 160 })
  @IsOptional()
  @IsString()
  @Length(0, 160)
  background_alt?: string;

  @ApiProperty({ required: false, minimum: 0, maximum: 100, default: 35 })
  @IsOptional()
  @IsInt()
  @Min(0)
  @Max(100)
  background_overlay?: number;

  @ApiProperty({ required: false, enum: BACKGROUND_POSICIONES, default: 'center' })
  @IsOptional()
  @IsIn(BACKGROUND_POSICIONES)
  background_position?: BackgroundPosition;

  @ApiProperty({ required: false, default: false })
  @IsOptional()
  @IsBoolean()
  background_activo?: boolean;
}

export class ActualizarSlugTiendaRequestDto {
  @ApiProperty({ minLength: 2, maxLength: 80, example: 'tienda-de-la-vendedora' })
  @IsString()
  @Length(2, 80)
  slug: string;
}

@ApiTags('Tiendas')
@ApiBearerAuth()
@Controller('api/v1/vendedores')
export class TiendasController {
  constructor(private readonly tiendas: TiendasService) {}

  @Post('tienda')
  @Roles(ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Crear la tienda del vendedor autenticado' })
  @ApiCreatedResponse({ type: TiendaResponseDto, description: 'Tienda creada' })
  async crear(@Body() dto: CrearTiendaRequestDto, @UsuarioActual() usuario: UsuarioContexto): Promise<Tienda> {
    if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
    const esEmpresario = usuario.rol === ROLES.EMPRESARIO;
    const participacion = esEmpresario ? null : usuario.participacion_porcentaje;
    if (!esEmpresario && participacion === undefined) {
      throw new NotFoundError('Participacion', 'del vendedor');
    }
    // El empresario crea tiendas disponibles (sin vendedor); el vendedor crea
    // su propia tienda operativa, conservando el flujo existente.
    const vendedorId = esEmpresario ? null : usuario.user_id;
    return this.tiendas.crear(
      usuario.tenant_id,
      usuario.user_id,
      participacion ?? null,
      dto.nombre,
      dto.descripcion,
      dto.slug,
      vendedorId,
    );
  }

  @Get('me/tienda')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Tienda del vendedor autenticado' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Tienda del vendedor' })
  async miTienda(@UsuarioActual() usuario: UsuarioContexto): Promise<Tienda> {
    const tienda = await this.tiendas.deVendedor(usuario.user_id);
    if (!tienda) throw new NotFoundError('Tienda', 'del vendedor');
    return tienda;
  }
}

/** GET /api/v1/empresarios/me/tiendas — tiendas de la empresa (ADR-25). */
@ApiTags('Tiendas')
@ApiBearerAuth()
@Controller('api/v1/empresarios/me')
export class TiendasEmpresaController {
  constructor(private readonly tiendas: TiendasService) {}

  @Get('tiendas')
  @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Tiendas de la empresa autenticada' })
  @ApiOkResponse({ type: [TiendaResponseDto], description: 'Tiendas de la empresa' })
  async listar(@UsuarioActual() usuario: UsuarioContexto): Promise<Tienda[]> {
    if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
    return this.tiendas.deEmpresa(usuario.tenant_id);
  }
}

/** PATCH /api/v1/tiendas/:id/slug — cambia el slug público (ADR-26). */
@ApiTags('Tiendas')
@ApiBearerAuth()
@Controller('api/v1/tiendas')
export class TiendasSlugController {
  constructor(private readonly tiendas: TiendasService) {}

  @Patch(':id/logo')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Asociar o quitar el logo público de la tienda' })
  @ApiParam({ name: 'id', description: 'UUID de la tienda o su slug actual' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Logo actualizado' })
  actualizarLogo(
    @Param('id') id: string,
    @Body() dto: ActualizarLogoTiendaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Tienda> {
    return this.tiendas.actualizarLogo(usuario, id, dto.logo_objeto_id ?? null);
  }

  @Patch(':id/background')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Asociar, cambiar o quitar el fondo público de la tienda' })
  @ApiParam({ name: 'id', description: 'UUID de la tienda o su slug actual' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Fondo actualizado' })
  actualizarBackground(
    @Param('id') id: string,
    @Body() dto: ActualizarBackgroundTiendaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Tienda> {
    return this.tiendas.actualizarBackground(usuario, id, dto);
  }

  @Patch(':id/slug')
  @Roles(ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Cambiar el slug público; los anteriores redirigen 301' })
  @ApiParam({ name: 'id', description: 'UUID de la tienda o su slug actual/anterior' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Slug actualizado' })
  @ApiResponse({ status: 403, description: 'El usuario no es propietario de la tienda' })
  @ApiResponse({ status: 409, description: 'El slug ya está en uso (incluye slugs históricos)' })
  actualizarSlug(
    @Param('id') id: string,
    @Body() dto: ActualizarSlugTiendaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Tienda> {
    return this.tiendas.actualizarSlug(usuario.user_id, id, dto.slug);
  }
}
