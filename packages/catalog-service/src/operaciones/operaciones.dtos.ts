import { ApiProperty, PartialType } from '@nestjs/swagger';
import { Type } from 'class-transformer';
import {
  ArrayMinSize,
  IsArray,
  IsBoolean,
  IsDateString,
  IsIn,
  IsInt,
  IsNumber,
  IsOptional,
  IsString,
  IsUUID,
  Matches,
  Max,
  Min,
  MinLength,
  ValidateNested,
} from 'class-validator';

export class ActualizarConfiguracionInventarioRequestDto {
  @ApiProperty({ required: false, enum: ['promedio', 'fifo', 'ueps'] })
  @IsOptional()
  @IsIn(['promedio', 'fifo', 'ueps'])
  metodo_costeo?: 'promedio' | 'fifo' | 'ueps';

  @ApiProperty({ required: false })
  @IsOptional()
  @IsBoolean()
  permitir_stock_negativo?: boolean;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsBoolean()
  alerta_stock_bajo?: boolean;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsBoolean()
  actualizar_costo_en_compra?: boolean;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  unidad_defecto?: string;
}

export class ImportarProductoFilaDto {
  @ApiProperty()
  @IsString()
  @Matches(/^[A-Za-z0-9-]{2,32}$/)
  sku: string;

  @ApiProperty()
  @IsString()
  @MinLength(2)
  nombre: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  categoria?: string;

  @ApiProperty({ example: '1000.00' })
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/)
  precio_base: string;

  @ApiProperty({ minimum: 0 })
  @IsInt()
  @Min(0)
  stock: number;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/)
  costo?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  marca?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  codigo_barras?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  unidad?: string;

  @ApiProperty({ required: false, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock_minimo?: number;

  @ApiProperty({ required: false, nullable: true, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock_maximo?: number | null;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsBoolean()
  es_materia_prima?: boolean;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  ubicacion?: string;
}

export class ImportarProductosRequestDto {
  @ApiProperty({ type: [ImportarProductoFilaDto], description: 'Máximo 200 filas' })
  @IsArray()
  @ArrayMinSize(1)
  @ValidateNested({ each: true })
  @Type(() => ImportarProductoFilaDto)
  filas: ImportarProductoFilaDto[];
}

export class ComboComponenteRequestDto {
  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  producto_id?: string;

  @ApiProperty({ description: 'SKU del producto componente' })
  @IsString()
  sku: string;

  @ApiProperty({ minimum: 1 })
  @IsInt()
  @Min(1)
  cantidad: number;
}

export class CrearComboRequestDto {
  @ApiProperty()
  @IsString()
  @Matches(/^[A-Za-z0-9-]{2,32}$/)
  sku: string;

  @ApiProperty()
  @IsString()
  @MinLength(2)
  nombre: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  codigo_barras?: string;

  @ApiProperty({ example: '2500.00' })
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/)
  precio: string;

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado?: 'activo' | 'inactivo';

  @ApiProperty({ type: [ComboComponenteRequestDto] })
  @IsArray()
  @ArrayMinSize(1)
  @ValidateNested({ each: true })
  @Type(() => ComboComponenteRequestDto)
  componentes: ComboComponenteRequestDto[];
}

export class ActualizarComboRequestDto {
  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  @MinLength(2)
  nombre?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  codigo_barras?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/)
  precio?: string;

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado?: 'activo' | 'inactivo';

  @ApiProperty({ required: false, type: [ComboComponenteRequestDto] })
  @IsOptional()
  @IsArray()
  @ArrayMinSize(1)
  @ValidateNested({ each: true })
  @Type(() => ComboComponenteRequestDto)
  componentes?: ComboComponenteRequestDto[];
}

export class CrearPromocionRequestDto {
  @ApiProperty()
  @IsString()
  @MinLength(2)
  nombre: string;

  @ApiProperty({ enum: ['2x1', 'descuento_cantidad', 'descuento_porcentaje', 'regalo'] })
  @IsIn(['2x1', 'descuento_cantidad', 'descuento_porcentaje', 'regalo'])
  tipo: '2x1' | 'descuento_cantidad' | 'descuento_porcentaje' | 'regalo';

  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  producto_id?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  sku?: string;

  @ApiProperty({ required: false, minimum: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  cantidad_requerida?: number | null;

  @ApiProperty({ required: false, minimum: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  cantidad_gratis?: number | null;

  @ApiProperty({ required: false, minimum: 1, maximum: 100 })
  @IsOptional()
  @IsInt()
  @Min(1)
  @Max(100)
  descuento_porcentaje?: number | null;

  @ApiProperty({ format: 'date' })
  @IsDateString()
  fecha_desde: string;

  @ApiProperty({ format: 'date' })
  @IsDateString()
  fecha_hasta: string;

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado?: 'activo' | 'inactivo';
}

export class ActualizarPromocionRequestDto extends PartialType(CrearPromocionRequestDto) {}

export class MovimientoLineaRequestDto {
  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  producto_id?: string;

  @ApiProperty()
  @IsString()
  sku: string;

  @ApiProperty({ minimum: 0 })
  @IsInt()
  @Min(0)
  cantidad: number;
}

export class CrearMovimientoInventarioRequestDto {
  @ApiProperty({ enum: ['entrada', 'salida', 'ajuste', 'traslado'] })
  @IsIn(['entrada', 'salida', 'ajuste', 'traslado'])
  tipo: 'entrada' | 'salida' | 'ajuste' | 'traslado';

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  motivo?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  referencia?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  ubicacion_origen?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  ubicacion_destino?: string;

  @ApiProperty({ type: [MovimientoLineaRequestDto] })
  @IsArray()
  @ArrayMinSize(1)
  @ValidateNested({ each: true })
  @Type(() => MovimientoLineaRequestDto)
  lineas: MovimientoLineaRequestDto[];
}

export class ListarCombosRequestDto {
  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  empresa_id?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  q?: string;

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado?: string;

  @ApiProperty({ required: false, minimum: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  pagina?: number;

  @ApiProperty({ required: false, minimum: 1, maximum: 100 })
  @IsOptional()
  @IsInt()
  @Min(1)
  @Max(100)
  limite?: number;
}

export class ListarPromocionesRequestDto {
  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  empresa_id?: string;

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado?: string;

  @ApiProperty({ required: false, minimum: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  pagina?: number;

  @ApiProperty({ required: false, minimum: 1, maximum: 100 })
  @IsOptional()
  @IsInt()
  @Min(1)
  @Max(100)
  limite?: number;
}

export class ListarMovimientosRequestDto {
  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  empresa_id?: string;

  @ApiProperty({ required: false, enum: ['entrada', 'salida', 'ajuste', 'traslado'] })
  @IsOptional()
  @IsIn(['entrada', 'salida', 'ajuste', 'traslado'])
  tipo?: string;

  @ApiProperty({ required: false, minimum: 1 })
  @IsOptional()
  @IsNumber()
  @Min(1)
  pagina?: number;

  @ApiProperty({ required: false, minimum: 1, maximum: 100 })
  @IsOptional()
  @IsNumber()
  @Min(1)
  @Max(100)
  limite?: number;
}
