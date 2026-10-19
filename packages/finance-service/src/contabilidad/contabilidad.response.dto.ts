import { ApiProperty } from '@nestjs/swagger';

export class DetalleAsientoResponseDto {
  @ApiProperty({ description: 'Código de cuenta contable' })
  cuenta_codigo: string;

  @ApiProperty({ description: 'Débito en centavos', example: 115000 })
  debe_cents: number;

  @ApiProperty({ description: 'Crédito en centavos', example: 115000 })
  haber_cents: number;

  @ApiProperty({ description: 'Concepto del detalle', required: false })
  concepto?: string;

  @ApiProperty({ description: 'Orden del detalle', required: false })
  orden?: number;
}

export class AsientoResponseDto {
  @ApiProperty({ description: 'Id del asiento', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Fecha del asiento', format: 'date-time' })
  fecha: string;

  @ApiProperty({ description: 'Concepto del asiento' })
  concepto: string;

  @ApiProperty({ description: 'Tipo de asiento', example: 'MANUAL' })
  tipo: string;

  @ApiProperty({ description: 'Tipo de referencia', required: false })
  referencia_tipo?: string;

  @ApiProperty({ description: 'Id de referencia', required: false })
  referencia_id?: string;

  @ApiProperty({ description: 'Moneda', example: 'C$' })
  moneda: string;

  @ApiProperty({ description: 'Estado del asiento', example: 'registrado' })
  estado: string;

  @ApiProperty({ description: 'Usuario que creó el asiento', required: false })
  creado_por?: string;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;

  @ApiProperty({ description: 'Total débito en centavos', example: 115000 })
  debe_cents: number;

  @ApiProperty({ description: 'Total crédito en centavos', example: 115000 })
  haber_cents: number;

  @ApiProperty({ description: 'Detalles del asiento', type: () => [DetalleAsientoResponseDto] })
  detalles: DetalleAsientoResponseDto[];
}

export class CuentaResponseDto {
  @ApiProperty({ description: 'Código de la cuenta', example: '1101' })
  codigo: string;

  @ApiProperty({ description: 'Nombre de la cuenta', example: 'Caja' })
  nombre: string;

  @ApiProperty({
    description: 'Tipo de cuenta',
    enum: ['ACTIVO', 'PASIVO', 'CAPITAL', 'INGRESO', 'COSTO', 'GASTO'],
    example: 'ACTIVO',
  })
  tipo: 'ACTIVO' | 'PASIVO' | 'CAPITAL' | 'INGRESO' | 'COSTO' | 'GASTO';

  @ApiProperty({ description: 'Naturaleza', enum: ['DEUDORA', 'ACREEDORA'], example: 'DEUDORA' })
  naturaleza: 'DEUDORA' | 'ACREEDORA';

  @ApiProperty({ description: 'Nivel jerárquico', example: 1 })
  nivel: number;

  @ApiProperty({ description: 'Estado', enum: ['activa', 'inactiva'], example: 'activa' })
  estado: 'activa' | 'inactiva';
}

export class LibroMayorResponseDto {
  @ApiProperty({ description: 'Cuenta contable', type: CuentaResponseDto, nullable: true })
  cuenta: CuentaResponseDto | null;

  @ApiProperty({ description: 'Saldo inicial en centavos', example: 100000 })
  saldo_inicial_cents: number;

  @ApiProperty({ description: 'Saldo final en centavos', example: 215000 })
  saldo_final_cents: number;

  @ApiProperty({ description: 'Movimientos del periodo', type: [Object] })
  movimientos: Record<string, unknown>[];
}

export class LibroIvaResponseDto {
  @ApiProperty({ description: 'Periodo (YYYY-MM)', example: '2026-10' })
  periodo: string;

  @ApiProperty({ description: 'Comprobantes del periodo', example: 12 })
  comprobantes: number;

  @ApiProperty({ description: 'Base gravada en centavos', example: 1150000 })
  base_gravada_cents: number;

  @ApiProperty({ description: 'IVA en centavos', example: 172500 })
  iva_cents: number;

  @ApiProperty({ description: 'Monto exento en centavos', example: 0 })
  exento_cents: number;

  @ApiProperty({ description: 'Total en centavos', example: 1322500 })
  total_cents: number;
}
