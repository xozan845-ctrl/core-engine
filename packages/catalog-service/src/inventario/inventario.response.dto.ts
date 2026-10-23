import { ApiProperty } from '@nestjs/swagger';

/**
 * Contrato de respuesta del inventario de bodega. `stock` es el stock físico,
 * `disponible` es el neto de reservas y `reservado`/`reservas_activas` el
 * importe reservado por órdenes activas.
 */
export class LineaInventarioResponseDto {
  @ApiProperty({ description: 'SKU del producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Nombre del producto' })
  nombre: string;

  @ApiProperty({ description: 'Stock físico en bodega', example: 12 })
  stock: number;

  @ApiProperty({ description: 'Stock disponible (neto de reservas)', example: 10 })
  disponible: number;

  @ApiProperty({ description: 'Unidades reservadas por órdenes activas', example: 2 })
  reservado: number;

  @ApiProperty({ description: 'Estado del producto', example: 'disponible' })
  estado: string;

  @ApiProperty({
    description: 'Precio base en córdobas con dos decimales',
    example: '1000.00',
  })
  precio_base: string;

  @ApiProperty({ description: 'Alias de reservado para consumidores internos', example: 2 })
  reservas_activas: number;

  @ApiProperty({
    description: 'Valor del stock disponible en centavos',
    example: 1000000,
  })
  valor_disponible_cents: number;

  @ApiProperty({
    description: 'Valor del stock físico total en centavos',
    example: 1200000,
  })
  valor_total_cents: number;
}

export class ResumenInventarioResponseDto {
  @ApiProperty({ description: 'Total de productos', example: 20 })
  productos: number;

  @ApiProperty({ description: 'Productos agotados', example: 2 })
  agotados: number;

  @ApiProperty({ description: 'Unidades en stock', example: 120 })
  stock_total: number;

  @ApiProperty({ description: 'Valor del inventario en centavos', example: 12000000 })
  valor_inventario_cents: number;
}
