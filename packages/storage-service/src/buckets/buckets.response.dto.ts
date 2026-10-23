import { ApiProperty } from '@nestjs/swagger';

export class EstadoBucketResponseDto {
  @ApiProperty({ description: 'Id del tenant', format: 'uuid' })
  tenant_id: string;

  @ApiProperty({ description: 'Nombre del bucket S3', example: 't-00000000-0000-0000-0000-000000000000' })
  bucket: string;

  @ApiProperty({ description: 'Cuota asignada en bytes', example: 5368709120 })
  cuota_bytes: number;

  @ApiProperty({ description: 'Bytes usados', example: 204800 })
  usado_bytes: number;

  @ApiProperty({ description: 'Bytes disponibles', example: 5368504320 })
  disponible_bytes: number;

  @ApiProperty({ description: 'Objetos activos', example: 12 })
  objetos: number;

  @ApiProperty({
    description: 'Estado del almacenamiento',
    enum: ['activo', 'suspendido'],
    example: 'activo',
  })
  estado: 'activo' | 'suspendido';

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;

  @ApiProperty({ description: 'Última actualización (ISO 8601 UTC)', format: 'date-time' })
  actualizado_en: string;
}

export class PaginaBucketsResponseDto {
  @ApiProperty({ description: 'Buckets de la página', type: () => [EstadoBucketResponseDto] })
  items: EstadoBucketResponseDto[];

  @ApiProperty({ description: 'Total de tenants con almacenamiento', example: 8 })
  total: number;

  @ApiProperty({ description: 'Página actual (1-based)', example: 1 })
  pagina: number;

  @ApiProperty({ description: 'Tamaño de página', example: 20 })
  limite: number;

  @ApiProperty({ description: 'Total de páginas', example: 1 })
  paginas: number;
}
