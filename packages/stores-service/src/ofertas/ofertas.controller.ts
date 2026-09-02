import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
import { IsInt, IsOptional, IsString, Max, Min } from 'class-validator';
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
  @IsString()
  producto_id: string;

  /** Margen en enteros (15 = 15 %). RN-01: 0 a 90. */
  @IsInt()
  @Min(0)
  @Max(90)
  margen: number;
}

export class CambiarMargenRequestDto {
  @IsInt()
  @Min(0)
  @Max(90)
  margen: number;
}

@Controller('api/v1')
export class OfertasController {
  constructor(
    private readonly tiendas: TiendasService,
    private readonly ofertas: OfertasService,
  ) {}

  /** POST /api/v1/vendedores/productos — publica producto con margen (Tabla 21). */
  @Post('vendedores/productos')
  @Roles(ROLES.VENDEDOR)
  async publicar(
    @Body() dto: PublicarProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Oferta> {
    return this.ofertas.publicar(usuario.user_id, dto.producto_id, dto.margen);
  }

  /** PATCH /api/v1/vendedores/productos/:id — cambia el margen de la oferta. */
  @Patch('vendedores/productos/:id')
  @Roles(ROLES.VENDEDOR)
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
  async misOfertas(
    @Query() query: { pagina?: string; limite?: string },
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Pagina<Oferta>> {
    return this.ofertas.deVendedor(usuario.user_id, query as unknown as Record<string, unknown>);
  }

  /** GET /api/v1/tiendas/:id — tienda publica con sus ofertas. */
  @Get('tiendas/:id')
  async tiendaPublica(@Param('id') tiendaId: string) {
    const tienda = await this.tiendas.encontrarPorId(tiendaId);
    if (!tienda) throw new NotFoundError('Tienda', tiendaId);
    const ofertas = await this.ofertas.deTienda(tiendaId);
    return { tienda, ofertas };
  }
}
