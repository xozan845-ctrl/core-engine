import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger';
import { IsIn, IsInt, IsOptional, IsString, Matches, Max, Min } from 'class-validator';
import {
  FUENTES,
  Fuente,
  MODOS,
  Modo,
  OrigenPersonalizacion,
  PLANTILLAS,
  Plantilla,
  Tema,
  TemaInput,
} from '@core/shared';

const COLOR_HEX = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

export class TemaRequestDto implements TemaInput {
  @ApiPropertyOptional({ description: 'Color primario (hex)', example: '#1F2937' })
  @IsOptional()
  @IsString()
  @Matches(COLOR_HEX)
  color_primario?: string;

  @ApiPropertyOptional({ description: 'Color secundario (hex)', example: '#F59E0B' })
  @IsOptional()
  @IsString()
  @Matches(COLOR_HEX)
  color_secundario?: string;

  @ApiPropertyOptional({ description: 'Color de acento (hex)', example: '#10B981' })
  @IsOptional()
  @IsString()
  @Matches(COLOR_HEX)
  color_acento?: string;

  @ApiPropertyOptional({ description: 'Color de fondo (hex)', example: '#FFFFFF' })
  @IsOptional()
  @IsString()
  @Matches(COLOR_HEX)
  color_fondo?: string;

  @ApiPropertyOptional({ description: 'Color de texto (hex)', example: '#111827' })
  @IsOptional()
  @IsString()
  @Matches(COLOR_HEX)
  color_texto?: string;

  @ApiPropertyOptional({ description: 'Fuente del tema', enum: FUENTES, default: 'system' })
  @IsOptional()
  @IsIn(FUENTES)
  fuente?: Fuente;

  @ApiPropertyOptional({
    description: 'Radio de bordes en píxeles',
    minimum: 0,
    maximum: 32,
    default: 12,
  })
  @IsOptional()
  @IsInt()
  @Min(0)
  @Max(32)
  radio_bordes?: number;

  @ApiPropertyOptional({ description: 'Modo de color', enum: MODOS, default: 'claro' })
  @IsOptional()
  @IsIn(MODOS)
  modo?: Modo;
}

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
  fuente: Fuente;

  @ApiProperty({ description: 'Radio de bordes en píxeles', minimum: 0, maximum: 32, example: 12 })
  radio_bordes: number;

  @ApiProperty({ description: 'Modo de color', enum: MODOS, example: 'claro' })
  modo: Modo;
}

export class OrigenPersonalizacionResponseDto {
  @ApiProperty({
    description: 'Nivel que aportó la plantilla efectiva',
    enum: ['tienda', 'empresa', 'plataforma', 'defecto'],
  })
  plantilla: OrigenPersonalizacion;

  @ApiProperty({
    description: 'Nivel más específico que aportó overrides de tema',
    enum: ['tienda', 'empresa', 'plataforma', 'defecto'],
  })
  tema: OrigenPersonalizacion;
}

export class TemaOverridesResponseDto {
  @ApiProperty({ description: 'Plantilla override o null si hereda', enum: PLANTILLAS, nullable: true })
  plantilla_override: Plantilla | null;

  @ApiProperty({
    description: 'Overrides parciales de tema',
    type: 'object',
    additionalProperties: true,
  })
  tema_override: TemaInput;
}
