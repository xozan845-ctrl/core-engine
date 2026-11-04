import { Body, Controller, Get, Param, Patch, Post } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiPropertyOptional,
  ApiResponse,
  ApiTags,
} from '@nestjs/swagger';
import { Type } from 'class-transformer';
import {
  ArrayMaxSize,
  IsArray,
  IsBoolean,
  IsIn,
  IsInt,
  IsOptional,
  IsString,
  IsUUID,
  Length,
  Matches,
  Max,
  Min,
  ValidateNested,
} from 'class-validator';
import { TiendasService, Tienda } from './tiendas.service';
import { TiendaResponseDto } from './tiendas.response.dto';
import { BACKGROUND_POSICIONES, BackgroundInput, BackgroundPosition } from './media-background';
import {
  FUENTES,
  Fuente,
  LIMITES_PERSONALIZACION,
  MODOS,
  Modo,
  PLANTILLAS,
  PersonalizacionInput,
  Plantilla,
  TemaInput,
} from './personalizacion-tienda';
import {
  ContenidoInput,
  LIMITES_CONTENIDO,
  Politicas,
  REDES_SOCIALES,
  RedSocial,
  RedSocialTipo,
  Seccion,
  TIPOS_SECCION,
  TipoSeccion,
} from './contenido-tienda';
import { Roles, ROLES, UsuarioActual, UsuarioContexto, NotFoundError } from '@core/shared';

const COLOR_HEX = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

export class CrearTiendaRequestDto {
  @ApiProperty({ description: 'Nombre de la tienda', minLength: 2, maxLength: 100, example: 'Mi tienda' })
  @IsString()
  @Length(2, 100)
  nombre: string;

  @ApiProperty({ description: 'Descripción de la tienda', required: false, example: 'Ropa y accesorios' })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({
    description: 'Slug público. Si se omite se deriva del nombre y se resuelven colisiones con sufijos.',
    required: false,
    minLength: 2,
    maxLength: 80,
    example: 'tienda-de-la-vendedora-piloto',
  })
  @IsOptional()
  @IsString()
  @Length(2, 80)
  slug?: string;
}

export class ActualizarLogoTiendaRequestDto {
  @ApiProperty({
    description: 'Objeto de storage público confirmado como imagen; null para quitar el logo',
    required: false,
    nullable: true,
    format: 'uuid',
  })
  @IsOptional()
  @IsUUID()
  logo_objeto_id?: string | null;
}

export class ActualizarBackgroundTiendaRequestDto implements BackgroundInput {
  @ApiProperty({
    description: 'Objeto de storage del fondo; null limpia el bloque completo',
    required: false,
    nullable: true,
    type: String,
    format: 'uuid',
  })
  @IsOptional()
  @IsUUID()
  background_objeto_id?: string | null;

  @ApiProperty({
    description: 'Imagen de portada para un fondo de video',
    required: false,
    nullable: true,
    type: String,
    format: 'uuid',
  })
  @IsOptional()
  @IsUUID()
  background_poster_objeto_id?: string | null;

  @ApiProperty({ required: false, maxLength: 160 })
  @IsOptional()
  @IsString()
  @Length(0, 160)
  background_alt?: string;

  @ApiProperty({ required: false, minimum: 0, maximum: 100, default: 35 })
  @IsOptional()
  @IsInt()
  @Min(0)
  @Max(100)
  background_overlay?: number;

  @ApiProperty({ required: false, enum: BACKGROUND_POSICIONES, default: 'center' })
  @IsOptional()
  @IsIn(BACKGROUND_POSICIONES)
  background_position?: BackgroundPosition;

  @ApiProperty({ required: false, default: false })
  @IsOptional()
  @IsBoolean()
  background_activo?: boolean;
}

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

  @ApiPropertyOptional({ description: 'Radio de bordes en píxeles', minimum: 0, maximum: 32, default: 12 })
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

export class ActualizarPersonalizacionTiendaRequestDto implements PersonalizacionInput {
  @ApiPropertyOptional({ description: 'Plantilla visual base', enum: PLANTILLAS, default: 'clasica' })
  @IsOptional()
  @IsIn(PLANTILLAS)
  plantilla?: Plantilla;

  @ApiPropertyOptional({ description: 'Tokens de tema; los campos ausentes conservan su valor', type: TemaRequestDto })
  @IsOptional()
  @ValidateNested()
  @Type(() => TemaRequestDto)
  tema?: TemaRequestDto;

  @ApiPropertyOptional({ maxLength: LIMITES_PERSONALIZACION.hero_titulo })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_PERSONALIZACION.hero_titulo)
  hero_titulo?: string;

  @ApiPropertyOptional({ maxLength: LIMITES_PERSONALIZACION.hero_subtitulo })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_PERSONALIZACION.hero_subtitulo)
  hero_subtitulo?: string;

  @ApiPropertyOptional({ maxLength: LIMITES_PERSONALIZACION.hero_cta_texto })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_PERSONALIZACION.hero_cta_texto)
  hero_cta_texto?: string;

  @ApiPropertyOptional({
    description: 'Ruta relativa o URL HTTPS del CTA',
    maxLength: LIMITES_PERSONALIZACION.hero_cta_url,
  })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_PERSONALIZACION.hero_cta_url)
  hero_cta_url?: string;

  @ApiPropertyOptional({ maxLength: LIMITES_PERSONALIZACION.seo_titulo })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_PERSONALIZACION.seo_titulo)
  seo_titulo?: string;

  @ApiPropertyOptional({ maxLength: LIMITES_PERSONALIZACION.seo_descripcion })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_PERSONALIZACION.seo_descripcion)
  seo_descripcion?: string;

  @ApiPropertyOptional({
    description: 'Objeto de storage público confirmado como imagen; null para quitarla',
    format: 'uuid',
    nullable: true,
  })
  @IsOptional()
  @IsUUID()
  seo_imagen_objeto_id?: string | null;
}

export class SeccionRequestDto implements Seccion {
  @ApiProperty({ description: 'Tipo de sección', enum: TIPOS_SECCION })
  @IsIn(TIPOS_SECCION)
  tipo: TipoSeccion;

  @ApiProperty({ description: 'Si la sección se muestra', default: true })
  @IsBoolean()
  visible: boolean;

  @ApiProperty({
    description: 'Orden de la sección',
    minimum: 0,
    maximum: LIMITES_CONTENIDO.secciones - 1,
  })
  @IsInt()
  @Min(0)
  @Max(LIMITES_CONTENIDO.secciones - 1)
  orden: number;
}

export class RedSocialRequestDto implements RedSocial {
  @ApiProperty({ description: 'Red social', enum: REDES_SOCIALES })
  @IsIn(REDES_SOCIALES)
  red: RedSocialTipo;

  @ApiProperty({ description: 'URL HTTPS del perfil', example: 'https://instagram.com/mitienda' })
  @IsString()
  @Length(1, 300)
  url: string;
}

export class PoliticasRequestDto implements Partial<Politicas> {
  @ApiPropertyOptional({ description: 'Política de envío', maxLength: LIMITES_CONTENIDO.politica })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_CONTENIDO.politica)
  envio?: string;

  @ApiPropertyOptional({
    description: 'Política de devolución',
    maxLength: LIMITES_CONTENIDO.politica,
  })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_CONTENIDO.politica)
  devolucion?: string;

  @ApiPropertyOptional({
    description: 'Política de privacidad',
    maxLength: LIMITES_CONTENIDO.politica,
  })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_CONTENIDO.politica)
  privacidad?: string;
}

export class ActualizarContenidoTiendaRequestDto implements ContenidoInput {
  @ApiPropertyOptional({
    description: 'Orden y visibilidad de las secciones; null o [] las limpia',
    type: [SeccionRequestDto],
    maxItems: LIMITES_CONTENIDO.secciones,
  })
  @IsOptional()
  @IsArray()
  @ArrayMaxSize(LIMITES_CONTENIDO.secciones)
  @ValidateNested({ each: true })
  @Type(() => SeccionRequestDto)
  secciones?: SeccionRequestDto[] | null;

  @ApiPropertyOptional({ description: 'Texto "sobre nosotros"', maxLength: LIMITES_CONTENIDO.sobre_nosotros })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_CONTENIDO.sobre_nosotros)
  sobre_nosotros?: string;

  @ApiPropertyOptional({ description: 'Políticas de la tienda', type: PoliticasRequestDto })
  @IsOptional()
  @ValidateNested()
  @Type(() => PoliticasRequestDto)
  politicas?: PoliticasRequestDto;

  @ApiPropertyOptional({
    description: 'Redes sociales; null o [] las limpia',
    type: [RedSocialRequestDto],
    maxItems: LIMITES_CONTENIDO.redes,
  })
  @IsOptional()
  @IsArray()
  @ArrayMaxSize(LIMITES_CONTENIDO.redes)
  @ValidateNested({ each: true })
  @Type(() => RedSocialRequestDto)
  redes?: RedSocialRequestDto[] | null;

  @ApiPropertyOptional({ description: 'Texto del anuncio superior', maxLength: LIMITES_CONTENIDO.anuncio_texto })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_CONTENIDO.anuncio_texto)
  anuncio_texto?: string;

  @ApiPropertyOptional({
    description: 'Ruta relativa o URL HTTPS del anuncio',
    maxLength: LIMITES_CONTENIDO.anuncio_enlace,
  })
  @IsOptional()
  @IsString()
  @Length(0, LIMITES_CONTENIDO.anuncio_enlace)
  anuncio_enlace?: string;

  @ApiPropertyOptional({ description: 'Si el anuncio está activo', default: false })
  @IsOptional()
  @IsBoolean()
  anuncio_activo?: boolean;
}

export class ActualizarSlugTiendaRequestDto {
  @ApiProperty({ minLength: 2, maxLength: 80, example: 'tienda-de-la-vendedora' })
  @IsString()
  @Length(2, 80)
  slug: string;
}

@ApiTags('Tiendas')
@ApiBearerAuth()
@Controller('api/v1/vendedores')
export class TiendasController {
  constructor(private readonly tiendas: TiendasService) {}

  @Post('tienda')
  @Roles(ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Crear la tienda del vendedor autenticado' })
  @ApiCreatedResponse({ type: TiendaResponseDto, description: 'Tienda creada' })
  async crear(@Body() dto: CrearTiendaRequestDto, @UsuarioActual() usuario: UsuarioContexto): Promise<Tienda> {
    if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
    const esEmpresario = usuario.rol === ROLES.EMPRESARIO;
    const participacion = esEmpresario ? null : usuario.participacion_porcentaje;
    if (!esEmpresario && participacion === undefined) {
      throw new NotFoundError('Participacion', 'del vendedor');
    }
    // El empresario crea tiendas disponibles (sin vendedor); el vendedor crea
    // su propia tienda operativa, conservando el flujo existente.
    const vendedorId = esEmpresario ? null : usuario.user_id;
    return this.tiendas.crear(
      usuario.tenant_id,
      usuario.user_id,
      participacion ?? null,
      dto.nombre,
      dto.descripcion,
      dto.slug,
      vendedorId,
    );
  }

  @Get('me/tienda')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Tienda del vendedor autenticado' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Tienda del vendedor' })
  async miTienda(@UsuarioActual() usuario: UsuarioContexto): Promise<Tienda> {
    const tienda = await this.tiendas.deVendedor(usuario.user_id);
    if (!tienda) throw new NotFoundError('Tienda', 'del vendedor');
    return tienda;
  }
}

/** GET /api/v1/empresarios/me/tiendas — tiendas de la empresa (ADR-25). */
@ApiTags('Tiendas')
@ApiBearerAuth()
@Controller('api/v1/empresarios/me')
export class TiendasEmpresaController {
  constructor(private readonly tiendas: TiendasService) {}

  @Get('tiendas')
  @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Tiendas de la empresa autenticada' })
  @ApiOkResponse({ type: [TiendaResponseDto], description: 'Tiendas de la empresa' })
  async listar(@UsuarioActual() usuario: UsuarioContexto): Promise<Tienda[]> {
    if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
    return this.tiendas.deEmpresa(usuario.tenant_id);
  }
}

/** PATCH /api/v1/tiendas/:id/slug — cambia el slug público (ADR-26). */
@ApiTags('Tiendas')
@ApiBearerAuth()
@Controller('api/v1/tiendas')
export class TiendasSlugController {
  constructor(private readonly tiendas: TiendasService) {}

  @Patch(':id/logo')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Asociar o quitar el logo público de la tienda' })
  @ApiParam({ name: 'id', description: 'UUID de la tienda o su slug actual' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Logo actualizado' })
  actualizarLogo(
    @Param('id') id: string,
    @Body() dto: ActualizarLogoTiendaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Tienda> {
    return this.tiendas.actualizarLogo(usuario, id, dto.logo_objeto_id ?? null);
  }

  @Patch(':id/background')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Asociar, cambiar o quitar el fondo público de la tienda' })
  @ApiParam({ name: 'id', description: 'UUID de la tienda o su slug actual' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Fondo actualizado' })
  actualizarBackground(
    @Param('id') id: string,
    @Body() dto: ActualizarBackgroundTiendaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Tienda> {
    return this.tiendas.actualizarBackground(usuario, id, dto);
  }

  @Patch(':id/personalizacion')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({
    summary: 'Actualizar la identidad visual de la tienda (plantilla, tema, hero y SEO)',
  })
  @ApiParam({ name: 'id', description: 'UUID de la tienda o su slug actual' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Personalización actualizada' })
  actualizarPersonalizacion(
    @Param('id') id: string,
    @Body() dto: ActualizarPersonalizacionTiendaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Tienda> {
    return this.tiendas.actualizarPersonalizacion(usuario, id, dto);
  }

  @Patch(':id/contenido')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({
    summary: 'Actualizar el contenido del storefront (secciones, políticas, redes y anuncio)',
  })
  @ApiParam({ name: 'id', description: 'UUID de la tienda o su slug actual' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Contenido actualizado' })
  actualizarContenido(
    @Param('id') id: string,
    @Body() dto: ActualizarContenidoTiendaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Tienda> {
    return this.tiendas.actualizarContenido(usuario, id, dto);
  }

  @Patch(':id/slug')
  @Roles(ROLES.VENDEDOR, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Cambiar el slug público; los anteriores redirigen 301' })
  @ApiParam({ name: 'id', description: 'UUID de la tienda o su slug actual/anterior' })
  @ApiOkResponse({ type: TiendaResponseDto, description: 'Slug actualizado' })
  @ApiResponse({ status: 403, description: 'El usuario no es propietario de la tienda' })
  @ApiResponse({ status: 409, description: 'El slug ya está en uso (incluye slugs históricos)' })
  actualizarSlug(
    @Param('id') id: string,
    @Body() dto: ActualizarSlugTiendaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Tienda> {
    return this.tiendas.actualizarSlug(usuario.user_id, id, dto.slug);
  }
}
