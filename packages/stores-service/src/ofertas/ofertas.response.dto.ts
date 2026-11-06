import { ApiProperty } from '@nestjs/swagger';
import { TiendaResponseDto } from '../tiendas/tiendas.response.dto';

export class OfertaResponseDto {
  @ApiProperty({ description: 'Id de la oferta', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Id de la tienda; solo propietario/admin', format: 'uuid', required: false })
  tienda_id: string;

  @ApiProperty({ description: 'Id del vendedor; solo propietario/admin', format: 'uuid', required: false })
  vendedor_id: string;

  @ApiProperty({ description: 'Empresa propietaria', format: 'uuid', required: false }) empresa_id: string;
  @ApiProperty({ description: 'Participación congelable del vendedor; solo propietario/admin', minimum: 0, maximum: 100, required: false }) participacion_porcentaje: number;

  @ApiProperty({ description: 'Id del producto o variante vendible', format: 'uuid' })
  producto_id: string;

  @ApiProperty({ description: 'Id del producto padre (si es variante)', required: false, nullable: true, format: 'uuid' })
  producto_padre_id: string | null;

  @ApiProperty({ description: 'SKU de la variante o producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Nombre del producto/variante' })
  producto_nombre: string;

  @ApiProperty({
    description: 'Atributos de la variante (color, talla, etc.)',
    type: 'object',
    additionalProperties: true,
    selfRequired: false,
  })
  atributos: Record<string, unknown>;

  @ApiProperty({ description: 'Id del objeto de la imagen principal', required: false, nullable: true, format: 'uuid' })
  imagen_objeto_id: string | null;

  @ApiProperty({ description: 'URL pública de la imagen principal', required: false, nullable: true })
  imagen_url: string | null;

  @ApiProperty({ description: 'Margen aplicado (0–90); solo propietario/admin', example: 15, required: false })
  margen: number;

  @ApiProperty({ description: 'Precio base; solo propietario/admin', example: '1000.00', required: false })
  precio_base: string;

  @ApiProperty({ description: 'Precio de venta en córdobas con dos decimales', example: '1150.00' })
  precio_venta: string;

  @ApiProperty({ description: 'Stock exacto; solo propietario/admin', example: 10, required: false })
  stock: number;

  @ApiProperty({
    description: 'Disponible para compra (derivado; no expone el stock exacto)',
    example: true,
    required: false,
  })
  disponible: boolean;

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
