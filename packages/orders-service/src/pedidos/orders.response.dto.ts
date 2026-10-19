import { ApiProperty } from '@nestjs/swagger';

export class ItemOrdenResponseDto {
  @ApiProperty({ description: 'Id de la oferta', format: 'uuid' })
  oferta_id: string;

  @ApiProperty({ description: 'SKU del producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Nombre del producto' })
  producto_nombre: string;

  @ApiProperty({ description: 'Cantidad pedida', example: 2 })
  cantidad: number;

  @ApiProperty({ description: 'Precio unitario de venta en centavos', example: 115000 })
  precio_unitario_cents: number;

  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'Id de la tienda', format: 'uuid' })
  tienda_id: string;
}

export class OrderViewResponseDto {
  @ApiProperty({ description: 'Id de la orden', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'Id del comprador', format: 'uuid' })
  comprador_id: string;

  @ApiProperty({ description: 'Id de la tienda', format: 'uuid' })
  tienda_id: string;

  @ApiProperty({ description: 'Estado de la orden', example: 'pagada' })
  estado: string;

  @ApiProperty({ description: 'Total en centavos', example: 230000 })
  total_cents: number;

  @ApiProperty({ description: 'Comisión en centavos', example: 27600 })
  comision_cents: number;

  @ApiProperty({ description: 'Moneda', example: 'C$' })
  moneda: string;

  @ApiProperty({ description: 'Líneas de la orden', type: () => [ItemOrdenResponseDto] })
  items: ItemOrdenResponseDto[];

  @ApiProperty({ description: 'Dirección de envío', required: false, nullable: true, type: Object })
  direccion_envio: Record<string, unknown> | null;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;

  @ApiProperty({ description: 'Última actualización (ISO 8601 UTC)', format: 'date-time' })
  actualizado_en: string;

  @ApiProperty({ description: 'Fecha de pago (ISO 8601 UTC)', required: false, nullable: true, format: 'date-time' })
  pagada_en: string | null;

  @ApiProperty({ description: 'Fecha de envío (ISO 8601 UTC)', required: false, nullable: true, format: 'date-time' })
  enviada_en: string | null;

  @ApiProperty({ description: 'Fecha de entrega (ISO 8601 UTC)', required: false, nullable: true, format: 'date-time' })
  entregada_en: string | null;

  @ApiProperty({ description: 'Fecha de cancelación (ISO 8601 UTC)', required: false, nullable: true, format: 'date-time' })
  cancelada_en: string | null;

  @ApiProperty({ description: 'Fecha de devolución (ISO 8601 UTC)', required: false, nullable: true, format: 'date-time' })
  devuelta_en: string | null;
}

export class ComisionVistaResponseDto {
  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'Periodo (YYYY-MM)', example: '2026-10' })
  periodo: string;

  @ApiProperty({ description: 'Órdenes del periodo', example: 12 })
  ordenes: number;

  @ApiProperty({ description: 'Ventas del periodo en centavos', example: 1380000 })
  total_ventas_cents: number;

  @ApiProperty({ description: 'Comisiones del periodo en centavos', example: 165600 })
  total_comision_cents: number;

  @ApiProperty({ description: 'Última actualización (ISO 8601 UTC)', format: 'date-time' })
  actualizado_en: string;
}

export class TimelineEventoResponseDto {
  @ApiProperty({ description: 'Secuencia del evento', example: 3 })
  id: number;

  @ApiProperty({ description: 'Tipo de evento', example: 'OrderStatusUpdatedEvent' })
  tipo: string;

  @ApiProperty({ description: 'Payload del evento', type: Object })
  payload: Record<string, unknown>;

  @ApiProperty({ description: 'Versión del agregado', example: 3 })
  version: number;

  @ApiProperty({ description: 'Fecha del evento (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}

export class HistoriaEventoResponseDto {
  @ApiProperty({ description: 'Id del evento', format: 'uuid' })
  event_id: string;

  @ApiProperty({ description: 'Id del agregado (orden)', format: 'uuid' })
  aggregate_id: string;

  @ApiProperty({ description: 'Tipo de evento', example: 'OrderCreatedEvent' })
  tipo: string;

  @ApiProperty({ description: 'Payload del evento', type: Object })
  payload: Record<string, unknown>;

  @ApiProperty({ description: 'Versión del agregado', example: 1 })
  version: number;

  @ApiProperty({ description: 'Fecha del evento (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}
