import { ApiProperty } from '@nestjs/swagger';

export class TiendaResponseDto {
  @ApiProperty({ description: 'Id de la tienda', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Empresa propietaria', format: 'uuid' }) empresa_id: string;
  @ApiProperty({ description: 'Usuario propietario operativo', format: 'uuid' }) propietario_usuario_id: string;
  @ApiProperty({ required: false, nullable: true, type: Number, minimum: 0, maximum: 100 })
  participacion_porcentaje: number | null;

  @ApiProperty({
    required: false,
    nullable: true,
    type: String,
    description: 'Id del vendedor asignado; null si la tienda está disponible',
    format: 'uuid',
  })
  vendedor_id: string | null;

  @ApiProperty({ description: 'Nombre de la tienda', example: 'Mi tienda' })
  nombre: string;

  // `required: false` mantiene el cambio como aditivo para el gate R-C-7;
  // el servicio siempre devuelve el slug en runtime.
  @ApiProperty({ required: false, description: 'Slug público actual', example: 'mi-tienda' })
  slug: string;

  @ApiProperty({ description: 'Descripción de la tienda' })
  descripcion: string;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}
