import { ApiProperty } from '@nestjs/swagger';

export class ComprobanteResponseDto {
  @ApiProperty({ description: 'Id del comprobante', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Serie', example: 'A' })
  serie: string;

  @ApiProperty({ description: 'Número', example: '000001' })
  numero: string;

  @ApiProperty({ description: 'Tipo', enum: ['FACTURA', 'NOTA_CREDITO'], example: 'FACTURA' })
  tipo: 'FACTURA' | 'NOTA_CREDITO';

  @ApiProperty({ description: 'Id de la orden', required: false, format: 'uuid' })
  orden_id?: string;

  @ApiProperty({ description: 'Id del cliente', required: false })
  cliente_id?: string;

  @ApiProperty({ description: 'Base gravada en centavos', example: 1150000 })
  base_gravada_cents: number;

  @ApiProperty({ description: 'IVA en centavos', example: 172500 })
  iva_cents: number;

  @ApiProperty({ description: 'Monto exento en centavos', example: 0 })
  exento_cents: number;

  @ApiProperty({ description: 'Total en centavos', example: 1322500 })
  total_cents: number;

  @ApiProperty({ description: 'Moneda', example: 'C$' })
  moneda: string;

  @ApiProperty({
    description: 'Estado',
    enum: ['BORRADOR', 'EMITIDO', 'ANULADO'],
    example: 'EMITIDO',
  })
  estado: 'BORRADOR' | 'EMITIDO' | 'ANULADO';

  @ApiProperty({
    description: 'Datos fiscales del cliente',
    required: false,
    nullable: true,
    type: Object,
  })
  datos_cliente?: Record<string, unknown> | null;

  @ApiProperty({ description: 'Fecha de emisión', required: false, format: 'date-time' })
  emitido_en?: string;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}

export class SerieComprobanteResponseDto {
  @ApiProperty({ description: 'Serie', example: 'A' })
  serie: string;

  @ApiProperty({ description: 'Tipo', enum: ['FACTURA', 'NOTA_CREDITO'], example: 'FACTURA' })
  tipo: 'FACTURA' | 'NOTA_CREDITO';

  @ApiProperty({ description: 'Prefijo', example: 'FAC' })
  prefijo: string;

  @ApiProperty({ description: 'Secuencial actual', example: 12 })
  secuencial_actual: number;

  @ApiProperty({ description: 'RUC del emisor', example: 'J0310000000000' })
  ruc_emisor: string;

  @ApiProperty({ description: 'Jurisdicción', example: 'NI' })
  jurisdiccion: string;

  @ApiProperty({ description: 'Serie activa', example: true })
  activa: boolean;
}
