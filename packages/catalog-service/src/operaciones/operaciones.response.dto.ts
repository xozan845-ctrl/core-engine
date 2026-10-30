import { ApiProperty } from '@nestjs/swagger';
import { ProductoResponseDto } from '../productos/productos.response.dto';
import { ServicioResponseDto } from '../servicios/servicios.response.dto';

export class ConfiguracionInventarioResponseDto {
  @ApiProperty({ format: 'uuid' })
  empresa_id: string;

  @ApiProperty({ enum: ['promedio', 'fifo', 'ueps'] })
  metodo_costeo: string;

  @ApiProperty()
  permitir_stock_negativo: boolean;

  @ApiProperty()
  alerta_stock_bajo: boolean;

  @ApiProperty()
  actualizar_costo_en_compra: boolean;

  @ApiProperty()
  unidad_defecto: string;

  @ApiProperty({ format: 'date-time' })
  actualizado_en: string;
}

export class ImportarProductosResponseDto {
  @ApiProperty()
  importados: number;

  @ApiProperty()
  omitidos: number;

  @ApiProperty({ type: [String] })
  errores: string[];
}

export class EscaneoResponseDto {
  @ApiProperty({ enum: ['producto', 'servicio'] })
  tipo: 'producto' | 'servicio';

  @ApiProperty({ type: ProductoResponseDto, required: false, nullable: true })
  producto: ProductoResponseDto | null;

  @ApiProperty({ type: ServicioResponseDto, required: false, nullable: true })
  servicio: ServicioResponseDto | null;
}

export class ComboComponenteResponseDto {
  @ApiProperty({ format: 'uuid' })
  producto_id: string;

  @ApiProperty()
  sku: string;

  @ApiProperty()
  nombre: string;

  @ApiProperty()
  cantidad: number;

  @ApiProperty()
  costo_unitario: string;

  @ApiProperty()
  subtotal: string;
}

export class ComboResponseDto {
  @ApiProperty({ format: 'uuid' })
  id: string;

  @ApiProperty({ format: 'uuid' })
  empresa_id: string;

  @ApiProperty()
  sku: string;

  @ApiProperty()
  nombre: string;

  @ApiProperty()
  descripcion: string;

  @ApiProperty()
  codigo_barras: string;

  @ApiProperty()
  precio: string;

  @ApiProperty()
  costo_total: string;

  @ApiProperty()
  margen: number;

  @ApiProperty({ enum: ['activo', 'inactivo'] })
  estado: 'activo' | 'inactivo';

  @ApiProperty({ type: [ComboComponenteResponseDto] })
  componentes: ComboComponenteResponseDto[];

  @ApiProperty({ format: 'date-time' })
  creado_en: string;

  @ApiProperty({ format: 'date-time' })
  actualizado_en: string;
}

export class PaginaCombosResponseDto {
  @ApiProperty({ type: [ComboResponseDto] })
  items: ComboResponseDto[];

  @ApiProperty()
  total: number;

  @ApiProperty()
  pagina: number;

  @ApiProperty()
  limite: number;

  @ApiProperty()
  paginas: number;
}

export class PromocionResponseDto {
  @ApiProperty({ format: 'uuid' })
  id: string;

  @ApiProperty({ format: 'uuid' })
  empresa_id: string;

  @ApiProperty()
  nombre: string;

  @ApiProperty({ enum: ['2x1', 'descuento_cantidad', 'descuento_porcentaje', 'regalo'] })
  tipo: string;

  @ApiProperty({ nullable: true })
  producto_id: string | null;

  @ApiProperty()
  sku: string;

  @ApiProperty({ nullable: true })
  cantidad_requerida: number | null;

  @ApiProperty({ nullable: true })
  cantidad_gratis: number | null;

  @ApiProperty({ nullable: true })
  descuento_porcentaje: number | null;

  @ApiProperty({ format: 'date' })
  fecha_desde: string;

  @ApiProperty({ format: 'date' })
  fecha_hasta: string;

  @ApiProperty({ enum: ['activo', 'inactivo'] })
  estado: 'activo' | 'inactivo';

  @ApiProperty({ format: 'date-time' })
  creado_en: string;

  @ApiProperty({ format: 'date-time' })
  actualizado_en: string;
}

export class PaginaPromocionesResponseDto {
  @ApiProperty({ type: [PromocionResponseDto] })
  items: PromocionResponseDto[];

  @ApiProperty()
  total: number;

  @ApiProperty()
  pagina: number;

  @ApiProperty()
  limite: number;

  @ApiProperty()
  paginas: number;
}

export class MovimientoInventarioResponseDto {
  @ApiProperty({ format: 'uuid' })
  id: string;

  @ApiProperty({ format: 'uuid' })
  empresa_id: string;

  @ApiProperty({ enum: ['entrada', 'salida', 'ajuste', 'traslado'] })
  tipo: string;

  @ApiProperty()
  motivo: string;

  @ApiProperty()
  referencia: string;

  @ApiProperty()
  ubicacion_origen: string;

  @ApiProperty()
  ubicacion_destino: string;

  @ApiProperty({ type: [Object] })
  lineas: Record<string, unknown>[];

  @ApiProperty({ nullable: true })
  creado_por: string | null;

  @ApiProperty({ format: 'date-time' })
  creado_en: string;
}

export class PaginaMovimientosResponseDto {
  @ApiProperty({ type: [MovimientoInventarioResponseDto] })
  items: MovimientoInventarioResponseDto[];

  @ApiProperty()
  total: number;

  @ApiProperty()
  pagina: number;

  @ApiProperty()
  limite: number;

  @ApiProperty()
  paginas: number;
}
