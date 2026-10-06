import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
import { ApiOperation, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
import { IsInt, IsString, Max, Min } from 'class-validator';
import { TiendasService } from '../tiendas/tiendas.service';
import { OfertasService, Oferta } from './ofertas.service';
import {
  Pagina,
  Roles,
  ROLES,
  UsuarioActual,
  UsuarioContexto,
  NotFoundError,
} from '@core/shared';

export class PublicarProductoRequestDto {
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
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Publicar un producto con margen (vendedor)' })
  @ApiResponse({ status: 201, description: 'Oferta creada' })
  async publicar(
    @Body() dto: PublicarProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Oferta> {
    return this.ofertas.publicar(usuario.user_id, dto.producto_id, dto.margen);
  }

  /** PATCH /api/v1/vendedores/productos/:id — cambia el margen de la oferta. */
  @Patch('vendedores/productos/:id')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Cambiar el margen de una oferta (vendedor)' })
  async cambiarMargen(
    @Param('id') id: string,
    @Body() dto: CambiarMargenRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Oferta> {
    return this.ofertas.cambiarMargen(id, usuario.user_id, dto.margen);
  }

  /** GET /api/v1/vendedores/me/ofertas — ofertas del vendedor autenticado. */
  @Get('vendedores/me/ofertas')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Listar las ofertas del vendedor autenticado' })
  async misOfertas(
    @Query() query: { pagina?: string; limite?: string },
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Pagina<Oferta>> {
    return this.ofertas.deVendedor(usuario.user_id, query as unknown as Record<string, unknown>);
  }

  /** GET /api/v1/tiendas/:id — tienda publica con sus ofertas. */
  @Get('tiendas/:id')
  @ApiOperation({ summary: 'Tienda pública con sus ofertas (ruta pública)' })
  @ApiResponse({ status: 404, description: 'Tienda inexistente (NO_ENCONTRADO)' })
  async tiendaPublica(@Param('id') tiendaId: string) {
    const tienda = await this.tiendas.encontrarPorId(tiendaId);
    if (!tienda) throw new NotFoundError('Tienda', tiendaId);
    const ofertas = await this.ofertas.deTienda(tiendaId);
    return { tienda, ofertas };
  }
}
