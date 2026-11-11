import { Body, Controller, Delete, Get, Param, Patch, Post, Query } from '@nestjs/common';
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
import { IsInt, Matches, Min } from 'class-validator';
import {
  CAPACIDADES,
  DomainError,
  RequiereCapacidades,
  ROLES,
  Roles,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import { CotizacionItem, DesgloseTarifa, TarifaItem, TarifasService } from './tarifas.service';

export class CrearTarifaRequestDto {
  @ApiProperty({ description: 'Inicio inclusive', format: 'date', example: '2026-12-20' })
  @Matches(/^\d{4}-\d{2}-\d{2}$/)
  desde: string;

  @ApiProperty({ description: 'Fin inclusive', format: 'date', example: '2026-12-31' })
  @Matches(/^\d{4}-\d{2}-\d{2}$/)
  hasta: string;

  @ApiProperty({ description: 'Precio por dia en centavos', minimum: 0, example: 25000 })
  @IsInt()
  @Min(0)
  precio_cents: number;
}

export class ActualizarTarifaRequestDto extends PartialType(CrearTarifaRequestDto) {}

export class TarifaItemResponseDto implements TarifaItem {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty({ format: 'date' }) desde: string;
  @ApiProperty({ format: 'date' }) hasta: string;
  @ApiProperty() precio_cents: number;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class DesgloseTarifaResponseDto implements DesgloseTarifa {
  @ApiProperty({ format: 'date' }) desde: string;
  @ApiProperty({ format: 'date' }) hasta: string;
  @ApiProperty() dias: number;
  @ApiProperty() precio_cents: number;
  @ApiProperty() subtotal_cents: number;
}

export class CotizacionItemResponseDto implements CotizacionItem {
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty({ format: 'date' }) desde: string;
  @ApiProperty({ format: 'date' }) hasta: string;
  @ApiProperty() dias: number;
  @ApiProperty() total_cents: number;
  @ApiProperty({ type: () => [DesgloseTarifaResponseDto] })
  desglose: DesgloseTarifa[];
}

@ApiTags('Catálogo · Tarifas')
@ApiBearerAuth()
@Controller('api/v2/catalog/items')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.CATALOGO_ITEMS)
export class TarifasController {
  constructor(private readonly tarifas: TarifasService) {}

  @Get(':id/tarifas')
  @ApiOperation({ summary: 'Listar tarifas por fecha de un item' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [TarifaItemResponseDto], description: 'Tarifas del item' })
  listar(
    @Param('id') itemId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.tarifas.listar(this.empresaDe(usuario, empresaId), itemId);
  }

  @Post(':id/tarifas')
  @ApiOperation({ summary: 'Crear una tarifa por rango de fechas' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: TarifaItemResponseDto, description: 'Tarifa creada' })
  crear(
    @Param('id') itemId: string,
    @Body() dto: CrearTarifaRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.tarifas.crear(this.empresaDe(usuario, empresaId), itemId, dto);
  }

  @Patch(':id/tarifas/:tarifaId')
  @ApiOperation({ summary: 'Actualizar una tarifa por fecha' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiParam({ name: 'tarifaId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: TarifaItemResponseDto, description: 'Tarifa actualizada' })
  actualizar(
    @Param('id') itemId: string,
    @Param('tarifaId') tarifaId: string,
    @Body() dto: ActualizarTarifaRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.tarifas.actualizar(this.empresaDe(usuario, empresaId), itemId, tarifaId, dto);
  }

  @Delete(':id/tarifas/:tarifaId')
  @ApiOperation({ summary: 'Eliminar una tarifa por fecha' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiParam({ name: 'tarifaId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ description: 'Tarifa eliminada' })
  eliminar(
    @Param('id') itemId: string,
    @Param('tarifaId') tarifaId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.tarifas.eliminar(this.empresaDe(usuario, empresaId), itemId, tarifaId);
  }

  @Get(':id/cotizacion')
  @ApiOperation({ summary: 'Cotizar un item por rango de fechas (tarifas + precio base)' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'desde', required: true, example: '2026-12-18' })
  @ApiQuery({ name: 'hasta', required: true, example: '2026-12-22' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: CotizacionItemResponseDto, description: 'Cotización del rango' })
  cotizar(
    @Param('id') itemId: string,
    @Query('desde') desde: string,
    @Query('hasta') hasta: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.tarifas.cotizar(this.empresaDe(usuario, empresaId), itemId, desde, hasta);
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
