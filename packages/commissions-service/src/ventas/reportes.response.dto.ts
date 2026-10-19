import { ApiProperty } from '@nestjs/swagger';

export class ReporteKpiResponseDto {
  @ApiProperty({ description: 'GMV del mes en centavos', example: 1150000 })
  gmv_mes_cents: number;

  @ApiProperty({ description: 'GMV del mes en córdobas con dos decimales', example: '11500.00' })
  gmv_mes: string;

  @ApiProperty({ description: 'Pedidos del mes', example: 42 })
  pedidos_mes: number;

  @ApiProperty({ description: 'Comisiones del mes en centavos', example: 138000 })
  comisiones_mes_cents: number;

  @ApiProperty({ description: 'Comisiones del mes en córdobas con dos decimales', example: '1380.00' })
  comisiones_mes: string;

  @ApiProperty({ description: 'Vendedores activos', example: 12 })
  vendedores_activos: number;

  @ApiProperty({ description: 'Productos en catálogo', example: 20 })
  productos: number;

  @ApiProperty({ description: 'Unidades en stock', example: 120 })
  stock_total: number;

  @ApiProperty({ description: 'Productos agotados', example: 2 })
  agotados: number;

  @ApiProperty({ description: 'Devoluciones del mes', example: 1 })
  devoluciones_mes: number;
}
