import { ApiProperty } from '@nestjs/swagger';
import {
  FUENTES,
  MODOS,
  PLANTILLAS,
  Plantilla,
  Tema,
} from './personalizacion-tienda';

export class TemaResponseDto implements Tema {
  @ApiProperty({ description: 'Color primario (hex)', example: '#1F2937' })
  color_primario: string;

  @ApiProperty({ description: 'Color secundario (hex)', example: '#F59E0B' })
  color_secundario: string;

  @ApiProperty({ description: 'Color de acento (hex)', example: '#10B981' })
  color_acento: string;

  @ApiProperty({ description: 'Color de fondo (hex)', example: '#FFFFFF' })
  color_fondo: string;

  @ApiProperty({ description: 'Color de texto (hex)', example: '#111827' })
  color_texto: string;

  @ApiProperty({ description: 'Fuente del tema', enum: FUENTES, example: 'system' })
  fuente: Tema['fuente'];

  @ApiProperty({ description: 'Radio de bordes en píxeles', minimum: 0, maximum: 32, example: 12 })
  radio_bordes: number;

  @ApiProperty({ description: 'Modo de color', enum: MODOS, example: 'claro' })
  modo: Tema['modo'];
}

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

  @ApiProperty({ description: 'Objeto de storage del logo', required: false, nullable: true, format: 'uuid' })
  logo_objeto_id?: string | null;

  @ApiProperty({ description: 'URL pública del logo', required: false, nullable: true })
  logo_url?: string | null;

  @ApiProperty({
    description: 'Objeto de storage del fondo',
    required: false,
    nullable: true,
    type: String,
    format: 'uuid',
  })
  background_objeto_id?: string | null;

  @ApiProperty({
    description: 'Tipo derivado del fondo',
    required: false,
    nullable: true,
    enum: ['imagen', 'video'],
  })
  background_media_type?: 'imagen' | 'video' | null;

  @ApiProperty({
    description: 'URL pública derivada del fondo',
    required: false,
    nullable: true,
    type: String,
  })
  background_url?: string | null;

  @ApiProperty({
    description: 'Objeto de storage del poster',
    required: false,
    nullable: true,
    type: String,
    format: 'uuid',
  })
  background_poster_objeto_id?: string | null;

  @ApiProperty({
    description: 'URL pública derivada del poster',
    required: false,
    nullable: true,
    type: String,
  })
  background_poster_url?: string | null;

  @ApiProperty({ description: 'Texto alternativo del fondo', required: false })
  background_alt?: string;

  @ApiProperty({ description: 'Opacidad del overlay (0-100)', required: false, minimum: 0, maximum: 100 })
  background_overlay?: number;

  @ApiProperty({
    description: 'Posición del fondo',
    required: false,
    enum: ['center', 'top', 'bottom', 'left', 'right'],
  })
  background_position?: 'center' | 'top' | 'bottom' | 'left' | 'right';

  @ApiProperty({ description: 'Si el fondo está activo', required: false })
  background_activo?: boolean;

  @ApiProperty({
    description: 'Plantilla visual base (ADR-39)',
    required: false,
    enum: PLANTILLAS,
  })
  plantilla?: Plantilla;

  @ApiProperty({
    description: 'Tokens de tema resueltos; siempre incluye todos los campos',
    required: false,
    type: TemaResponseDto,
  })
  tema?: TemaResponseDto;

  @ApiProperty({ description: 'Título del hero', required: false })
  hero_titulo?: string;

  @ApiProperty({ description: 'Subtítulo del hero', required: false })
  hero_subtitulo?: string;

  @ApiProperty({ description: 'Texto del CTA del hero', required: false })
  hero_cta_texto?: string;

  @ApiProperty({ description: 'Ruta o URL HTTPS del CTA del hero', required: false })
  hero_cta_url?: string;

  @ApiProperty({ description: 'Título SEO de la tienda', required: false })
  seo_titulo?: string;

  @ApiProperty({ description: 'Descripción SEO de la tienda', required: false })
  seo_descripcion?: string;

  @ApiProperty({
    description: 'Objeto de storage de la imagen SEO',
    required: false,
    nullable: true,
    format: 'uuid',
  })
  seo_imagen_objeto_id?: string | null;

  @ApiProperty({
    description: 'URL pública derivada de la imagen SEO',
    required: false,
    nullable: true,
  })
  seo_imagen_url?: string | null;

  @ApiProperty({
    description: 'Última actualización de la personalización',
    required: false,
    nullable: true,
    format: 'date-time',
  })
  personalizacion_actualizada_en?: string | null;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;

  @ApiProperty({ description: 'Última actualización (ISO 8601 UTC)', required: false, format: 'date-time' })
  actualizado_en?: string;
}
