import { Controller, Get, Param, Query, UseGuards } from '@nestjs/common';
import { ApiOkResponse, ApiOperation, ApiParam, ApiQuery, ApiSecurity, ApiTags } from '@nestjs/swagger';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { InventarioService } from '../inventario/inventario.service';
import { ItemsService } from '../items/items.service';
import { ItemResponseDto, LoteItemResponseDto } from '../items/items.response.dto';
import { TarifasService } from '../items/tarifas.service';
import { CotizacionItemResponseDto } from '../items/tarifas.controller';
import {
  LineaInventarioResponseDto,
  ResumenInventarioResponseDto,
} from '../inventario/inventario.response.dto';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna.
 * Desde la Fase 3 la única superficie es `items`: productos, servicios y
 * alquileres comparten las mismas rutas.
 */
@ApiTags('Catálogo (interno)')
@ApiSecurity('internalKey')
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(
    private readonly inventario: InventarioService,
    private readonly items: ItemsService,
    private readonly tarifas: TarifasService,
  ) {}

  @Get('items/lote')
  @ApiOperation({ summary: 'Lote de items por códigos (servicio→servicio)' })
  @ApiQuery({ name: 'skus', required: false, description: 'Códigos separados por coma' })
  @ApiOkResponse({ type: [LoteItemResponseDto], description: 'Items encontrados' })
  async lote(@Query('skus') skus?: string) {
    const lista = (skus ?? '')
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
    return this.items.lotePorSkus(lista);
  }

  @Get('items/:id')
  @ApiOperation({ summary: 'Item unificado por id (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id del item' })
  @ApiOkResponse({ type: ItemResponseDto, description: 'Item encontrado' })
  async itemPorId(@Param('id') id: string) {
    const item = await this.items.encontrarPorId(id);
    if (!item) throw new NotFoundError('Item', id);
    return item;
  }

  @Get('items/:id/cotizacion')
  @ApiOperation({ summary: 'Cotizar un item por rango de fechas (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id del item' })
  @ApiQuery({ name: 'empresa_id', required: true })
  @ApiQuery({ name: 'desde', required: true, example: '2026-12-20' })
  @ApiQuery({ name: 'hasta', required: true, example: '2026-12-22' })
  @ApiOkResponse({ type: CotizacionItemResponseDto, description: 'Cotización del rango' })
  async cotizar(
    @Param('id') id: string,
    @Query('empresa_id') empresaId: string,
    @Query('desde') desde: string,
    @Query('hasta') hasta: string,
  ) {
    return this.tarifas.cotizar(empresaId, id, desde, hasta);
  }

  @Get('inventario/resumen')
  @ApiOperation({ summary: 'Resumen de inventario (servicio→servicio)' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Filtra el resumen por empresa (ADR-27)' })
  @ApiOkResponse({ type: ResumenInventarioResponseDto, description: 'Resumen de inventario' })
  async resumen(@Query('empresa_id') empresaId?: string) {
    return this.inventario.resumen(empresaId);
  }

  @Get('inventario/lineas')
  @ApiOperation({ summary: 'Líneas de inventario (servicio→servicio)' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Filtra las líneas por empresa (ADR-27)' })
  @ApiOkResponse({ type: [LineaInventarioResponseDto], description: 'Líneas de inventario' })
  async lineas(@Query('empresa_id') empresaId?: string) {
    return this.inventario.listar(empresaId);
  }
}
