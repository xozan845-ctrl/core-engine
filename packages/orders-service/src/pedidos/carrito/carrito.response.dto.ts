import { ApiProperty } from '@nestjs/swagger';

export class CarritoItemResponseDto {
  @ApiProperty({ description: 'Id de la oferta', format: 'uuid' })
  oferta_id: string;

  @ApiProperty({ description: 'Cantidad', example: 2 })
  cantidad: number;

  @ApiProperty({ description: 'Id de la oferta (alias enriquecido)', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Id de la tienda', format: 'uuid' })
  tienda_id: string;

  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'SKU del producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Nombre del producto' })
  producto_nombre: string;

  @ApiProperty({ description: 'Precio de venta en córdobas con dos decimales', example: '1150.00' })
  precio_venta: string;

  @ApiProperty({ description: 'Stock disponible', example: 10 })
  stock: number;

  @ApiProperty({ description: 'Estado de la oferta', example: 'activa' })
  estado: string;
}

export class CarritoResponseDto {
  @ApiProperty({ description: 'Items del carrito', type: () => [CarritoItemResponseDto] })
  items: CarritoItemResponseDto[];

  @ApiProperty({ description: 'Total del carrito en córdobas con dos decimales', example: '2300.00' })
  total: string;

  @ApiProperty({
    description: 'Expiración del carrito (ISO 8601 UTC); null si está vacío',
    required: false,
    nullable: true,
    format: 'date-time',
  })
  expira_en: string | null;
}
