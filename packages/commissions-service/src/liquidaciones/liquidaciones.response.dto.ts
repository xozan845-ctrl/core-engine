import { ApiProperty } from '@nestjs/swagger';

export class LiquidacionResponseDto {
  @ApiProperty({ description: 'Id de la liquidación', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'Inicio del periodo (ISO 8601)', format: 'date' })
  periodo_inicio: string;

  @ApiProperty({ description: 'Fin del periodo (ISO 8601)', format: 'date' })
  periodo_fin: string;

  @ApiProperty({ description: 'Monto liquidado en centavos', example: 13800 })
  monto_cents: number;

  @ApiProperty({
    description: 'Estado de la liquidación',
    enum: ['aprobada', 'pagada'],
    example: 'aprobada',
  })
  estado: 'aprobada' | 'pagada';

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}

export class LiquidacionConMontoResponseDto extends LiquidacionResponseDto {
  @ApiProperty({ description: 'Monto liquidado en córdobas con dos decimales', example: '138.00' })
  monto: string;
}
