import { ApiPropertyOptional } from '@nestjs/swagger';
import {
  ArrayMinSize,
  IsArray,
  IsIn,
  IsInt,
  IsLatitude,
  IsLongitude,
  IsNumber,
  IsOptional,
  IsString,
  IsUUID,
  Max,
  Min,
} from 'class-validator';

/**
 * Ingesta manual de una venta geolocalizada. Refleja el Data Contract
 * `VentaGeolocalizadaSchema`; el `vendedor_id` del cuerpo solo lo usan los
 * roles administrativos, porque un vendedor autenticado solo puede registrar
 * sus propias ventas.
 */
export class RegistrarVentaRequestDto {
  // `required: false` en OpenAPI conserva la compatibilidad del contrato
  // anterior (body libre); class-validator sigue exigiendo estos campos en
  // runtime porque el Data Contract los marca obligatorios.
  @ApiPropertyOptional({ format: 'uuid', description: 'Obligatorio en runtime' })
  @IsUUID()
  order_id: string;

  @ApiPropertyOptional({ format: 'uuid', description: 'Obligatorio en runtime' })
  @IsUUID()
  vendedor_id: string;

  @ApiPropertyOptional({ type: [String], minItems: 1, description: 'Obligatorio en runtime' })
  @IsArray()
  @ArrayMinSize(1)
  @IsString({ each: true })
  skus: string[];

  @ApiPropertyOptional({ example: 115000, minimum: 0, description: 'Obligatorio en runtime' })
  @IsInt()
  @Min(0)
  monto_cents: number;

  @ApiPropertyOptional({ example: 12.136 })
  @IsOptional()
  @IsLatitude()
  lat?: number;

  @ApiPropertyOptional({ example: -86.251 })
  @IsOptional()
  @IsLongitude()
  lng?: number;

  @ApiPropertyOptional({ minimum: 0 })
  @IsOptional()
  @IsNumber()
  @Min(0)
  precision?: number;

  @ApiPropertyOptional({ minimum: 0 })
  @IsOptional()
  @IsNumber()
  @Min(0)
  velocidad?: number;

  @ApiPropertyOptional({ minimum: 0, maximum: 360 })
  @IsOptional()
  @IsNumber()
  @Min(0)
  @Max(360)
  rumbo?: number;

  @ApiPropertyOptional({ enum: ['impulsacion', 'venta', 'entrega', 'reparto'] })
  @IsOptional()
  @IsIn(['impulsacion', 'venta', 'entrega', 'reparto'])
  tipo_actividad?: 'impulsacion' | 'venta' | 'entrega' | 'reparto';

  @ApiPropertyOptional({ enum: ['visitado', 'cerca_no_visitado', 'no_visitado'] })
  @IsOptional()
  @IsIn(['visitado', 'cerca_no_visitado', 'no_visitado'])
  resultado_visita?: 'visitado' | 'cerca_no_visitado' | 'no_visitado';

  @ApiPropertyOptional({ minimum: 0 })
  @IsOptional()
  @IsNumber()
  @Min(0)
  distancia_cliente_metros?: number;

  @ApiPropertyOptional({ enum: ['18-24', '25-34', '35-44', '45-54', '55+'] })
  @IsOptional()
  @IsIn(['18-24', '25-34', '35-44', '45-54', '55+'])
  rango_edad?: '18-24' | '25-34' | '35-44' | '45-54' | '55+';

  @ApiPropertyOptional({ enum: ['M', 'F', 'NS'] })
  @IsOptional()
  @IsIn(['M', 'F', 'NS'])
  genero?: 'M' | 'F' | 'NS';

  @ApiPropertyOptional({
    format: 'uuid',
    description: 'Clave de idempotencia; si falta se usa order_id.',
  })
  @IsOptional()
  @IsUUID()
  evento_id?: string;
}
