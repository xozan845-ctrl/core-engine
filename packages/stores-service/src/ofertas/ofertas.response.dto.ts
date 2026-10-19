import { ApiProperty } from '@nestjs/swagger';
import { TiendaResponseDto } from '../tiendas/tiendas.response.dto';

export class OfertaResponseDto {
  @ApiProperty({ description: 'Id de la oferta', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Id de la tienda', format: 'uuid' })
  tienda_id: string;

  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'Id del producto maestro', format: 'uuid' })
  producto_id: string;

  @ApiProperty({ description: 'SKU del producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Nombre del producto' })
  producto_nombre: string;

  @ApiProperty({ description: 'Margen aplicado (0–90)', example: 15 })
  margen: number;

  @ApiProperty({ description: 'Precio base en córdobas con dos decimales', example: '1000.00' })
  precio_base: string;

  @ApiProperty({ description: 'Precio de venta en córdobas con dos decimales', example: '1150.00' })
  precio_venta: string;

  @ApiProperty({ description: 'Stock disponible', example: 10 })
  stock: number;

  @ApiProperty({ description: 'Estado de la oferta', example: 'activa' })
  estado: string;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}

export class PaginaOfertasResponseDto {
  @ApiProperty({ description: 'Ofertas de la página', type: () => [OfertaResponseDto] })
  items: OfertaResponseDto[];

  @ApiProperty({ description: 'Total de ofertas', example: 42 })
  total: number;

  @ApiProperty({ description: 'Página actual (1-based)', example: 1 })
  pagina: number;

  @ApiProperty({ description: 'Tamaño de página', example: 20 })
  limite: number;

  @ApiProperty({ description: 'Total de páginas', example: 3 })
  paginas: number;
}

/** Tienda pública con sus ofertas (GET /api/v1/tiendas/:id). */
export class TiendaPublicaResponseDto {
  @ApiProperty({ description: 'Datos de la tienda', type: TiendaResponseDto })
  tienda: TiendaResponseDto;

  @ApiProperty({ description: 'Ofertas de la tienda', type: () => [OfertaResponseDto] })
  ofertas: OfertaResponseDto[];
}
