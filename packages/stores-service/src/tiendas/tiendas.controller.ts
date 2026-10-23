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
import { IsOptional, IsString, Length } from 'class-validator';
import { TiendasService, Tienda } from './tiendas.service';
import { TiendaResponseDto } from './tiendas.response.dto';
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
    const participacion = usuario.rol === ROLES.EMPRESARIO ? 100 : usuario.participacion_porcentaje;
    if (participacion === undefined) throw new NotFoundError('Participacion', 'del vendedor');
    return this.tiendas.crear(usuario.tenant_id, usuario.user_id, participacion, dto.nombre, dto.descripcion, dto.slug);
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
