import { ApiProperty } from '@nestjs/swagger';

export class ItemImagenResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) objeto_id: string;
  @ApiProperty() orden: number;
  @ApiProperty() es_principal: boolean;
}

export class ItemResponseDto {
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ enum: ['producto', 'servicio', 'alquiler'] }) tipo: 'producto' | 'servicio' | 'alquiler';
  @ApiProperty() codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() descripcion: string;
  @ApiProperty() categoria: string;
  @ApiProperty({ example: '1500.00' }) precio: string;
  @ApiProperty({ example: '500.00' }) costo: string;
  @ApiProperty() track_inventory: boolean;
  @ApiProperty({ nullable: true }) stock: number | null;
  @ApiProperty({ nullable: true }) duration_minutes: number | null;
  @ApiProperty({ type: 'object', additionalProperties: true }) atributos: Record<string, unknown>;
  @ApiProperty({ nullable: true, format: 'uuid' }) parent_item_id: string | null;
  @ApiProperty() marca: string;
  @ApiProperty() codigo_barras: string;
  @ApiProperty() unidad: string;
  @ApiProperty() permite_fracciones: boolean;
  @ApiProperty() unidad_compra: string;
  @ApiProperty() contenido_cantidad: number;
  @ApiProperty() contenido_unidad: string;
  @ApiProperty({ enum: ['fijo', 'escala'] }) tipo_precio: 'fijo' | 'escala';
  @ApiProperty({ enum: ['fijo', 'escala'] }) tipo_costo: 'fijo' | 'escala';
  @ApiProperty() stock_minimo: number;
  @ApiProperty({ nullable: true }) stock_maximo: number | null;
  @ApiProperty({ nullable: true }) garantia_meses: number | null;
  @ApiProperty() es_materia_prima: boolean;
  @ApiProperty() ubicacion: string;
  @ApiProperty({ type: () => [ItemImagenResponseDto] }) imagenes: ItemImagenResponseDto[];
  @ApiProperty({ enum: ['activo', 'inactivo'] }) estado: 'activo' | 'inactivo';
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class PaginaItemsResponseDto {
  @ApiProperty({ type: () => [ItemResponseDto] }) items: ItemResponseDto[];
  @ApiProperty() total: number;
  @ApiProperty() pagina: number;
  @ApiProperty() limite: number;
  @ApiProperty() paginas: number;
}

export class ItemSchemaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ nullable: true, format: 'uuid' }) empresa_id: string | null;
  @ApiProperty({ enum: ['producto', 'servicio', 'alquiler'] }) tipo: 'producto' | 'servicio' | 'alquiler';
  @ApiProperty() categoria: string;
  @ApiProperty({ type: 'object', additionalProperties: true }) esquema: Record<string, unknown>;
  @ApiProperty() version: number;
  @ApiProperty() activo: boolean;
}

export class EsquemaGuardadoResponseDto {
  @ApiProperty() ok: boolean;
}

export class LoteItemResponseDto {
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty() sku: string;
  @ApiProperty() stock: number;
  @ApiProperty({ enum: ['disponible', 'agotado'] }) estado: string;
}
