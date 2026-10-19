import { ApiProperty } from '@nestjs/swagger';
import { LiquidacionConMontoResponseDto } from '../liquidaciones/liquidaciones.response.dto';

export class VentaVendedorResponseDto {
  @ApiProperty({ description: 'Id de la orden', format: 'uuid' })
  order_id: string;

  @ApiProperty({ description: 'Monto de la venta en córdobas con dos decimales', example: '1150.00' })
  venta: string;

  @ApiProperty({ description: 'Comisión en córdobas con dos decimales', example: '138.00' })
  comision: string;

  @ApiProperty({ description: 'Monto neto para el vendedor', example: '1012.00' })
  monto_vendedor: string;

  @ApiProperty({ description: 'Estado de la comisión', example: 'acreditada' })
  estado: string;

  @ApiProperty({ description: 'Fecha de la venta (ISO 8601 UTC)', format: 'date-time' })
  fecha: string;
}

export class EstadoCuentaResponseDto {
  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'Ventas del vendedor', type: () => [VentaVendedorResponseDto] })
  ventas: VentaVendedorResponseDto[];

  @ApiProperty({ description: 'Comisiones pendientes en córdobas', example: '138.00' })
  comisiones_pendientes: string;

  @ApiProperty({
    description: 'Liquidaciones del vendedor',
    type: () => [LiquidacionConMontoResponseDto],
  })
  liquidaciones: LiquidacionConMontoResponseDto[];
}
