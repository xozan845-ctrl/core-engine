import { ApiProperty } from '@nestjs/swagger';

export class ObjetoStorageResponseDto {
  @ApiProperty({ description: 'Id del objeto', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Id del tenant dueño', format: 'uuid' })
  tenant_id: string;

  @ApiProperty({ description: 'Bucket S3', example: 't-00000000-0000-0000-0000-000000000000' })
  bucket: string;

  @ApiProperty({ description: 'Clave del objeto en el bucket', example: 'productos/foto.jpg' })
  objeto: string;

  @ApiProperty({ description: 'Nombre original del archivo', example: 'foto-producto.jpg' })
  nombre: string;

  @ApiProperty({ description: 'Content-Type', example: 'image/jpeg' })
  content_type: string;

  @ApiProperty({ description: 'Carpeta lógica', example: 'productos' })
  carpeta: string;

  @ApiProperty({ description: 'Descripción libre' })
  descripcion: string;

  @ApiProperty({
    description: 'Etiquetas clave-valor',
    type: 'object',
    additionalProperties: { type: 'string' },
  })
  etiquetas: Record<string, string>;

  @ApiProperty({ description: 'Tamaño en bytes', example: 204800 })
  tamano_bytes: number;

  @ApiProperty({ description: 'ETag devuelto por MinIO', required: false, nullable: true })
  etag: string | null;

  @ApiProperty({
    description: 'Estado del objeto',
    enum: ['pendiente', 'activo', 'eliminado'],
    example: 'activo',
  })
  estado: 'pendiente' | 'activo' | 'eliminado';

  @ApiProperty({ description: 'Usuario que subió el objeto', required: false, nullable: true })
  subido_por: string | null;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;

  @ApiProperty({
    description: 'Fecha de confirmación (ISO 8601 UTC)',
    required: false,
    nullable: true,
    format: 'date-time',
  })
  confirmado_en: string | null;

  @ApiProperty({
    description: 'Fecha de eliminación lógica (ISO 8601 UTC)',
    required: false,
    nullable: true,
    format: 'date-time',
  })
  eliminado_en: string | null;
}

export class PaginaObjetosResponseDto {
  @ApiProperty({ description: 'Objetos de la página', type: () => [ObjetoStorageResponseDto] })
  items: ObjetoStorageResponseDto[];

  @ApiProperty({ description: 'Total de objetos', example: 12 })
  total: number;

  @ApiProperty({ description: 'Página actual (1-based)', example: 1 })
  pagina: number;

  @ApiProperty({ description: 'Tamaño de página', example: 20 })
  limite: number;

  @ApiProperty({ description: 'Total de páginas', example: 1 })
  paginas: number;
}

export class SolicitudSubidaResponseDto {
  @ApiProperty({ description: 'Id del objeto pendiente', format: 'uuid' })
  objeto_id: string;

  @ApiProperty({ description: 'Bucket S3' })
  bucket: string;

  @ApiProperty({ description: 'Clave del objeto en el bucket' })
  objeto: string;

  @ApiProperty({ description: 'URL prefirmada para el PUT directo', format: 'uri' })
  url_subida: string;

  @ApiProperty({ description: 'Método HTTP de subida', enum: ['PUT'], example: 'PUT' })
  metodo: 'PUT';

  @ApiProperty({
    description: 'Cabeceras que debe enviar el cliente',
    type: 'object',
    additionalProperties: { type: 'string' },
  })
  headers: Record<string, string>;

  @ApiProperty({ description: 'Expiración de la URL (ISO 8601 UTC)', format: 'date-time' })
  expira_en: string;

  @ApiProperty({ description: 'Tamaño máximo permitido en bytes', example: 204800 })
  max_bytes: number;
}

export class UrlDescargaResponseDto {
  @ApiProperty({ description: 'URL prefirmada de descarga', format: 'uri' })
  url_descarga: string;

  @ApiProperty({ description: 'Expiración de la URL (ISO 8601 UTC)', format: 'date-time' })
  expira_en: string;

  @ApiProperty({ description: 'Nombre original del archivo' })
  nombre: string;

  @ApiProperty({ description: 'Content-Type', example: 'image/jpeg' })
  content_type: string;

  @ApiProperty({ description: 'Tamaño en bytes', example: 204800 })
  tamano_bytes: number;
}
