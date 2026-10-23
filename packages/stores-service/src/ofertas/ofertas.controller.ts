import { Body, Controller, Get, Param, Patch, Post, Query, Res } from '@nestjs/common';
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
import type { Response } from 'express';
import { IsInt, IsOptional, IsString, Max, Min } from 'class-validator';
import { TiendasService } from '../tiendas/tiendas.service';
import { OfertasService, Oferta } from './ofertas.service';
import {
  OfertaResponseDto,
  PaginaOfertasResponseDto,
  TiendaPublicaResponseDto,
} from './ofertas.response.dto';
import {
  Pagina,
  Roles,
  ROLES,
  UsuarioActual,
  UsuarioContexto,
  NotFoundError,
} from '@core/shared';

export class PublicarProductoRequestDto {
  @ApiProperty({
    description: 'Tienda donde se publicará (UUID o slug). Si se omite, se usa la tienda del vendedor autenticado.',
    required: false,
    example: 'mi-tienda',
  })
  @IsOptional()
  @IsString()
  tienda_id?: string;

  @ApiProperty({ description: 'Id del producto a publicar', example: 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11' })
  @IsString()
  producto_id: string;

  /** Margen en enteros (15 = 15 %). RN-01: 0 a 90. */
  @ApiProperty({ description: 'Margen en enteros (15 = 15 %). RN-01: 0 a 90', minimum: 0, maximum: 90, example: 15 })
  @IsInt()
  @Min(0)
  @Max(90)
  margen: number;
}

export class CambiarMargenRequestDto {
  @ApiProperty({ description: 'Nuevo margen en enteros (0 a 90)', minimum: 0, maximum: 90, example: 20 })
  @IsInt()
  @Min(0)
  @Max(90)
  margen: number;
}

@ApiTags('Tiendas')
@Controller('api/v1')
export class OfertasController {
  constructor(
    private readonly tiendas: TiendasService,
    private readonly ofertas: OfertasService,
  ) {}

  /** POST /api/v1/vendedores/productos — publica producto con margen (Tabla 21). */
  @Post('vendedores/productos')
  @Roles(ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Publicar un producto con margen (vendedor)' })
  @ApiCreatedResponse({ type: OfertaResponseDto, description: 'Oferta creada' })
  async publicar(
    @Body() dto: PublicarProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Oferta> {
    let tiendaId: string | undefined;
    if (dto.tienda_id) {
      const tienda = await this.tiendas.encontrarPorId(dto.tienda_id);
      if (!tienda) throw new NotFoundError('Tienda', dto.tienda_id);
      tiendaId = tienda.id;
    } else {
      tiendaId = (await this.tiendas.deVendedor(usuario.user_id))?.id;
    }
    if (!tiendaId) throw new NotFoundError('Tienda del vendedor', usuario.user_id);
    return this.ofertas.publicar(usuario.user_id, tiendaId, dto.producto_id, dto.margen);
  }

  /** PATCH /api/v1/vendedores/productos/:id — cambia el margen de la oferta. */
  @Patch('vendedores/productos/:id')
  @Roles(ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Cambiar el margen de una oferta (vendedor)' })
  @ApiOkResponse({ type: OfertaResponseDto, description: 'Oferta actualizada' })
  async cambiarMargen(
    @Param('id') id: string,
    @Body() dto: CambiarMargenRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Oferta> {
    return this.ofertas.cambiarMargen(id, usuario.user_id, dto.margen);
  }

  /** GET /api/v1/vendedores/me/ofertas — ofertas del vendedor autenticado. */
  @Get('vendedores/me/ofertas')
  @Roles(ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Listar las ofertas del vendedor autenticado' })
  @ApiOkResponse({ type: PaginaOfertasResponseDto, description: 'Página de ofertas' })
  async misOfertas(
    @Query() query: { pagina?: string; limite?: string },
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Pagina<Oferta>> {
    return this.ofertas.deVendedor(usuario.user_id, query as unknown as Record<string, unknown>);
  }

  /** GET /api/v1/tiendas/:id — tienda publica con sus ofertas (ruta pública). */
  @Get('tiendas/:id')
  @ApiOperation({ summary: 'Tienda pública con sus ofertas (UUID o slug; ruta pública)' })
  @ApiParam({
    name: 'id',
    description: 'UUID de la tienda o su slug público actual/anterior. Un slug anterior responde 301.',
    example: 'mi-tienda',
  })
  @ApiOkResponse({ type: TiendaPublicaResponseDto, description: 'Tienda con sus ofertas' })
  @ApiResponse({
    status: 301,
    description: 'Slug anterior; Location apunta al slug actual',
    headers: { Location: { description: 'Ruta con el slug actual', schema: { type: 'string' } } },
  })
  @ApiResponse({ status: 404, description: 'Tienda inexistente (NO_ENCONTRADO)' })
  async tiendaPublica(
    @Param('id') tiendaId: string,
    @Res({ passthrough: true }) res: Response,
  ) {
    const resuelta = await this.tiendas.resolver(tiendaId);
    if (!resuelta) throw new NotFoundError('Tienda', tiendaId);
    if (resuelta.redirigirA) res.status(301).setHeader('Location', resuelta.redirigirA);
    const ofertas = await this.ofertas.deTienda(resuelta.tienda.id);
    return { tienda: resuelta.tienda, ofertas };
  }
}
