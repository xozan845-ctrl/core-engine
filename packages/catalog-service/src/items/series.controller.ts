import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiQuery,
  ApiTags,
  PartialType,
} from '@nestjs/swagger';
import { IsBoolean, IsIn, IsObject, IsOptional, IsUUID, Matches } from 'class-validator';
import {
  CAPACIDADES,
  DomainError,
  RequiereCapacidades,
  ROLES,
  Roles,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import {
  CrearItemSerieInput,
  ESTADOS_SERIE,
  EstadoSerie,
  ItemSerie,
  ItemSerieDetalle,
  SeriesService,
} from './series.service';

const SERIAL = /^[A-Za-z0-9._-]{1,120}$/;
const IMEI = /^[0-9]{0,20}$/;

export class CrearItemSerieRequestDto {
  @ApiProperty({ example: 'SN-2026-0001' })
  @Matches(SERIAL)
  serial: string;

  @ApiProperty({ required: false, example: '356789012345678' })
  @IsOptional()
  @Matches(IMEI)
  imei?: string;

  @ApiProperty({ required: false, nullable: true, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  ubicacion_id?: string | null;

  @ApiProperty({ required: false, type: Object, additionalProperties: true })
  @IsOptional()
  @IsObject()
  metadata?: Record<string, unknown>;
}

export class ActualizarItemSerieRequestDto extends PartialType(CrearItemSerieRequestDto) {
  @ApiProperty({ required: false, enum: ['disponible', 'baja'] })
  @IsOptional()
  @IsIn(['disponible', 'baja'])
  estado?: EstadoSerie;
}

export class HabilitarSerializacionItemRequestDto {
  @ApiProperty()
  @IsBoolean()
  activo: boolean;
}

export class ItemSerieResponseDto implements ItemSerie {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty({ format: 'uuid', nullable: true }) ubicacion_id: string | null;
  @ApiProperty() serial: string;
  @ApiProperty() imei: string;
  @ApiProperty({ enum: ESTADOS_SERIE }) estado: EstadoSerie;
  @ApiProperty({ format: 'uuid', nullable: true }) order_id: string | null;
  @ApiProperty({ format: 'date-time', nullable: true }) vendido_en: string | null;
  @ApiProperty({ format: 'date', nullable: true }) garantia_hasta: string | null;
  @ApiProperty({ format: 'date-time', nullable: true }) devuelto_en: string | null;
  @ApiProperty({ type: Object, additionalProperties: true })
  metadata: Record<string, unknown>;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class ItemSerieDetalleResponseDto extends ItemSerieResponseDto implements ItemSerieDetalle {
  @ApiProperty() sku: string;
  @ApiProperty() nombre_item: string;
  @ApiProperty({ nullable: true }) garantia_meses: number | null;
  @ApiProperty() garantia_vigente: boolean;
}

export class HabilitarSerializacionItemResponseDto {
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty() track_serial: boolean;
}

@ApiTags('Catálogo · Series')
@ApiBearerAuth()
@Controller('api/v2/catalog')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.CATALOGO_ITEMS)
export class SeriesController {
  constructor(private readonly series: SeriesService) {}

  @Get('items/:id/series')
  @ApiOperation({ summary: 'Listar las unidades serializadas de un item' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [ItemSerieResponseDto], description: 'Unidades del item' })
  listar(
    @Param('id') itemId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.series.listar(this.empresaDe(usuario, empresaId), itemId);
  }

  @Post('items/:id/series')
  @ApiOperation({ summary: 'Registrar una unidad serializada' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: ItemSerieResponseDto, description: 'Unidad registrada' })
  crear(
    @Param('id') itemId: string,
    @Body() dto: CrearItemSerieRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.series.crear(this.empresaDe(usuario, empresaId), itemId, dto as CrearItemSerieInput);
  }

  @Patch('items/:id/serializacion')
  @ApiOperation({ summary: 'Activar o desactivar la serializacion de un item' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({
    type: HabilitarSerializacionItemResponseDto,
    description: 'Estado de serializacion',
  })
  habilitar(
    @Param('id') itemId: string,
    @Body() dto: HabilitarSerializacionItemRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.series.habilitarSerializacion(
      this.empresaDe(usuario, empresaId),
      itemId,
      dto.activo,
    );
  }

  @Patch('series/:serieId')
  @ApiOperation({ summary: 'Actualizar una unidad serializada' })
  @ApiParam({ name: 'serieId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: ItemSerieResponseDto, description: 'Unidad actualizada' })
  actualizar(
    @Param('serieId') serieId: string,
    @Body() dto: ActualizarItemSerieRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.series.actualizar(this.empresaDe(usuario, empresaId), serieId, dto);
  }

  @Get('series/:serial')
  @ApiOperation({ summary: 'Consultar una unidad por serial y su garantia' })
  @ApiParam({ name: 'serial' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: ItemSerieDetalleResponseDto, description: 'Unidad y garantia' })
  buscar(
    @Param('serial') serial: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.series.buscarPorSerial(this.empresaDe(usuario, empresaId), serial);
  }

  private empresaDe(usuario: UsuarioContexto, solicitada?: string): string {
    if (usuario.rol === ROLES.EMPRESARIO || usuario.rol_empresa_id) {
      if (!usuario.tenant_id) throw new DomainError('EMPRESA_REQUERIDA', 'No hay empresa activa.');
      if (solicitada && solicitada !== usuario.tenant_id) {
        throw new DomainError('EMPRESA_NO_AUTORIZADA', 'No puedes operar otra empresa.');
      }
      return usuario.tenant_id;
    }
    if (!solicitada) throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id.');
    return solicitada;
  }
}
