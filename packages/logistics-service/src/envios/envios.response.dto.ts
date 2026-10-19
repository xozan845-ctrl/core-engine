import { ApiProperty } from '@nestjs/swagger';

export class ItemOrdenExternaResponseDto {
  @ApiProperty({ description: 'SKU del producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Cantidad pedida', example: 2 })
  cantidad: number;

  @ApiProperty({ description: 'Nombre del producto' })
  producto_nombre: string;
}

/** Orden devuelta por orders-service al avanzar el estado. */
export class OrdenExternaResponseDto {
  @ApiProperty({ description: 'Id de la orden', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Id del comprador', format: 'uuid' })
  cliente_id: string;

  @ApiProperty({ description: 'Líneas de la orden', type: () => [ItemOrdenExternaResponseDto] })
  items: ItemOrdenExternaResponseDto[];

  @ApiProperty({ description: 'Total en córdobas con dos decimales', example: '1150.00' })
  total: string;

  @ApiProperty({ description: 'Total en centavos', example: 115000 })
  total_cents: number;

  @ApiProperty({ description: 'Estado actual de la orden', example: 'enviada' })
  estado: string;
}

export class EnvioResponseDto {
  @ApiProperty({ description: 'Id de la guía de despacho', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Número de guía', example: 'BGH-ABC123-DEF456' })
  guia: string;

  @ApiProperty({ description: 'Id de la orden asociada', format: 'uuid' })
  order_id: string;

  @ApiProperty({ description: 'Estado del envío', example: 'en_preparacion' })
  estado: string;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}

export class EnvioConMontoResponseDto extends EnvioResponseDto {
  @ApiProperty({ description: 'Monto del envío en córdobas con dos decimales', example: '1150.00' })
  monto: string;
}
