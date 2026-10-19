import { ApiProperty } from '@nestjs/swagger';

export class JurisdiccionResponseDto {
  @ApiProperty({ description: 'Código ISO del país', example: 'NI' })
  codigo_pais: string;

  @ApiProperty({ description: 'Nombre de la jurisdicción', example: 'Nicaragua' })
  nombre: string;

  @ApiProperty({ description: 'Moneda', example: 'NIO' })
  moneda: string;

  @ApiProperty({ description: 'Símbolo de la moneda', example: 'C$' })
  simbolo_moneda: string;

  @ApiProperty({ description: 'Tasa de IVA (0–1)', example: 0.15 })
  tasa_iva: number;

  @ApiProperty({ description: 'Tasa de IR (0–1)', example: 0.3 })
  tasa_ir: number;

  @ApiProperty({ description: 'Periodicidad de declaración', example: 'MENSUAL' })
  periodicidad_declaracion: string;

  @ApiProperty({
    description: 'Leyes aplicables (clave→texto)',
    type: 'object',
    additionalProperties: { type: 'string' },
  })
  leyes: Record<string, string>;

  @ApiProperty({ description: 'Jurisdicción activa', example: true })
  activa: boolean;
}

export class RegimenFiscalResponseDto {
  @ApiProperty({ description: 'Id del régimen', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Jurisdicción', example: 'NI' })
  jurisdiccion: string;

  @ApiProperty({ description: 'Código del régimen', example: 'CUOTA_FIJA' })
  codigo: string;

  @ApiProperty({ description: 'Nombre del régimen', example: 'Cuota Fija' })
  nombre: string;

  @ApiProperty({ description: 'Descripción' })
  descripcion: string;

  @ApiProperty({ description: 'Periodicidad', example: 'MENSUAL' })
  periodicidad: string;

  @ApiProperty({ description: 'Condición de ingresos anuales en centavos', required: false })
  condicion_ingresos_anuales_cents?: number;

  @ApiProperty({ description: 'Estado', enum: ['activo', 'inactivo'], example: 'activo' })
  estado: 'activo' | 'inactivo';
}

export class SujetoTributarioResponseDto {
  @ApiProperty({ description: 'Id del sujeto', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Jurisdicción', example: 'NI' })
  jurisdiccion: string;

  @ApiProperty({ description: 'Id del régimen', required: false, format: 'uuid' })
  regimen_id?: string;

  @ApiProperty({ description: 'Nombre del régimen', required: false })
  regimen_nombre?: string;

  @ApiProperty({ description: 'Razón social' })
  razon_social: string;

  @ApiProperty({ description: 'RUC', required: false })
  ruc?: string;

  @ApiProperty({ description: '¿Es la plataforma?', example: false })
  es_plataforma: boolean;

  @ApiProperty({ description: 'Estado', enum: ['ACTIVO', 'BAJA'], example: 'ACTIVO' })
  estado: 'ACTIVO' | 'BAJA';

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}

export class DeclaracionResponseDto {
  @ApiProperty({ description: 'Id de la declaración', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Jurisdicción', example: 'NI' })
  jurisdiccion: string;

  @ApiProperty({ description: 'Tipo', enum: ['IVA', 'IR', 'CUOTA_FIJA'], example: 'IVA' })
  tipo: 'IVA' | 'IR' | 'CUOTA_FIJA';

  @ApiProperty({ description: 'Inicio del periodo', format: 'date' })
  periodo_inicio: string;

  @ApiProperty({ description: 'Fin del periodo', format: 'date' })
  periodo_fin: string;

  @ApiProperty({ description: 'Base gravada en centavos', example: 1150000 })
  base_gravada_cents: number;

  @ApiProperty({ description: 'IVA debitado en centavos', example: 172500 })
  iva_debitado_cents: number;

  @ApiProperty({ description: 'IVA crédito en centavos', example: 0 })
  iva_credito_cents: number;

  @ApiProperty({ description: 'IVA a pagar en centavos', example: 172500 })
  iva_a_pagar_cents: number;

  @ApiProperty({ description: 'Renta bruta en centavos', example: 1150000 })
  renta_bruta_cents: number;

  @ApiProperty({ description: 'Renta gravable en centavos', example: 575000 })
  renta_gravable_cents: number;

  @ApiProperty({ description: 'IR a pagar en centavos', example: 172500 })
  ir_a_pagar_cents: number;

  @ApiProperty({ description: 'Cuota fija en centavos', example: 5000 })
  cuota_cents: number;

  @ApiProperty({
    description: 'Estado',
    enum: ['PENDIENTE', 'GENERADA', 'PRESENTADA', 'PAGADA', 'CANCELADA'],
    example: 'GENERADA',
  })
  estado: 'PENDIENTE' | 'GENERADA' | 'PRESENTADA' | 'PAGADA' | 'CANCELADA';

  @ApiProperty({ description: 'Detalle de la declaración', required: false, nullable: true, type: Object })
  detalle?: Record<string, unknown> | null;

  @ApiProperty({ description: 'Fecha de generación', required: false, format: 'date-time' })
  generada_en?: string;

  @ApiProperty({ description: 'Fecha de presentación', required: false, format: 'date-time' })
  presentada_en?: string;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}
