import { Body, Controller, Get, Patch, Res } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiOkResponse,
  ApiOperation,
  ApiProperty,
  ApiPropertyOptional,
  ApiTags,
} from '@nestjs/swagger';
import type { Response } from 'express';
import { IsBoolean, IsIn, IsInt, IsOptional, IsString, IsUUID, Length, Max, Min } from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import {
  BACKGROUND_POSICIONES,
  BackgroundPosition,
} from '../media-background/media-background';
import {
  ActualizarConfiguracionSitioInput,
  ConfiguracionSitioService,
} from './configuracion-sitio.service';

export class ActualizarConfiguracionSitioRequestDto implements ActualizarConfiguracionSitioInput {
  @ApiPropertyOptional({
    type: String,
    format: 'uuid',
    nullable: true,
    description: 'Objeto de storage del fondo; null limpia todo el bloque',
  })
  @IsOptional()
  @IsUUID()
  background_objeto_id?: string | null;

  @ApiPropertyOptional({
    type: String,
    format: 'uuid',
    nullable: true,
    description: 'Imagen de portada para un fondo de video',
  })
  @IsOptional()
  @IsUUID()
  background_poster_objeto_id?: string | null;

  @ApiPropertyOptional({ maxLength: 160, default: '' })
  @IsOptional()
  @IsString()
  @Length(0, 160)
  background_alt?: string;

  @ApiPropertyOptional({ minimum: 0, maximum: 100, default: 35 })
  @IsOptional()
  @IsInt()
  @Min(0)
  @Max(100)
  background_overlay?: number;

  @ApiPropertyOptional({ enum: BACKGROUND_POSICIONES, default: 'center' })
  @IsOptional()
  @IsIn(BACKGROUND_POSICIONES)
  background_position?: BackgroundPosition;

  @ApiPropertyOptional({ default: false })
  @IsOptional()
  @IsBoolean()
  background_activo?: boolean;
}

export class ConfiguracionSitioResponseDto {
  @ApiProperty({ type: String, format: 'uuid', nullable: true }) background_objeto_id: string | null;
  @ApiProperty({ enum: ['imagen', 'video'], nullable: true }) background_media_type:
    | 'imagen'
    | 'video'
    | null;
  @ApiProperty({ type: String, nullable: true, description: 'URL pública derivada del fondo' })
  background_url: string | null;
  @ApiProperty({ type: String, format: 'uuid', nullable: true })
  background_poster_objeto_id: string | null;
  @ApiProperty({ type: String, nullable: true, description: 'URL pública derivada del poster' })
  background_poster_url: string | null;
  @ApiProperty() background_alt: string;
  @ApiProperty({ minimum: 0, maximum: 100 }) background_overlay: number;
  @ApiProperty({ enum: BACKGROUND_POSICIONES }) background_position: BackgroundPosition;
  @ApiProperty() background_activo: boolean;
  @ApiProperty({ type: String, format: 'date-time', nullable: true })
  actualizado_en: string | null;
}

@ApiTags('Configuración del sitio')
@Controller('api/v1/configuracion/sitio')
export class ConfiguracionSitioController {
  constructor(private readonly configuracion: ConfiguracionSitioService) {}

  @Get()
  @ApiOperation({ summary: 'Configuración visible del sitio (fondo de plataforma)' })
  @ApiOkResponse({
    type: ConfiguracionSitioResponseDto,
    description: 'Fondo de plataforma y metadatos de presentación',
  })
  async obtener(@Res({ passthrough: true }) res: Response) {
    res.setHeader('Cache-Control', 'public, max-age=60');
    return this.configuracion.obtener();
  }

  @Patch()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Actualizar el fondo y los metadatos del sitio (admin)' })
  @ApiOkResponse({
    type: ConfiguracionSitioResponseDto,
    description: 'Configuración del sitio actualizada',
  })
  actualizar(
    @Body() dto: ActualizarConfiguracionSitioRequestDto,
    @UsuarioActual() u: UsuarioContexto,
  ) {
    return this.configuracion.actualizar(dto, u.user_id);
  }
}
