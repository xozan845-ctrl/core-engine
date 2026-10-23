import { ApiProperty } from '@nestjs/swagger';

export class TiendaResponseDto {
  @ApiProperty({ description: 'Id de la tienda', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Empresa propietaria', format: 'uuid' }) empresa_id: string;
  @ApiProperty({ description: 'Usuario propietario operativo', format: 'uuid' }) propietario_usuario_id: string;
  @ApiProperty({ minimum: 0, maximum: 100 }) participacion_porcentaje: number;

  @ApiProperty({ description: 'Id del vendedor dueño', format: 'uuid' })
  vendedor_id: string;

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
