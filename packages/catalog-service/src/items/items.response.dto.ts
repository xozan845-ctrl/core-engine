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
  @ApiProperty({ type: () => [ItemImagenResponseDto] }) imagenes: ItemImagenResponseDto[];
  @ApiProperty({ enum: ['activo', 'inactivo'] }) estado: 'activo' | 'inactivo';
  @ApiProperty({ nullable: true }) source_type: 'producto' | 'servicio' | null;
  @ApiProperty({ nullable: true, format: 'uuid' }) source_id: string | null;
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
