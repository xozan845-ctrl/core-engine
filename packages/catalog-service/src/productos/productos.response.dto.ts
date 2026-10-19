import { ApiProperty } from '@nestjs/swagger';

/**
 * Contrato de respuesta del catálogo. Refleja el shape que serializa
 * `ProductosService` (precio en string decimal, stock neto y estado derivado).
 */
export class ProductoResponseDto {
  @ApiProperty({ description: 'Id del producto', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'SKU único del producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Nombre del producto' })
  nombre: string;

  @ApiProperty({ description: 'Descripción del producto' })
  descripcion: string;

  @ApiProperty({ description: 'Categoría del producto' })
  categoria: string;

  @ApiProperty({
    description: 'Precio base en córdobas con dos decimales',
    example: '1000.00',
  })
  precio_base: string;

  @ApiProperty({ description: 'Stock disponible (neto de reservas)', example: 10 })
  stock: number;

  @ApiProperty({
    description: 'Estado del producto',
    enum: ['disponible', 'agotado'],
    example: 'disponible',
  })
  estado: 'disponible' | 'agotado';

  @ApiProperty({
    description: 'Fecha de creación (ISO 8601 UTC)',
    format: 'date-time',
  })
  creado_en: string;
}

export class PaginaProductosResponseDto {
  @ApiProperty({ description: 'Productos de la página', type: () => [ProductoResponseDto] })
  items: ProductoResponseDto[];

  @ApiProperty({ description: 'Total de productos que cumplen el filtro', example: 42 })
  total: number;

  @ApiProperty({ description: 'Página actual (1-based)', example: 1 })
  pagina: number;

  @ApiProperty({ description: 'Tamaño de página', example: 20 })
  limite: number;

  @ApiProperty({ description: 'Total de páginas', example: 3 })
  paginas: number;
}

export class LoteProductoResponseDto {
  @ApiProperty({ description: 'SKU del producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Stock disponible', example: 10 })
  stock: number;

  @ApiProperty({
    description: 'Estado del producto',
    enum: ['disponible', 'agotado'],
    example: 'disponible',
  })
  estado: string;
}
