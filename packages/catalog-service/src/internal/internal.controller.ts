import { Body, Controller, Get, Param, Post, Query, UseGuards } from '@nestjs/common';
import { ApiOkResponse, ApiOperation, ApiParam, ApiQuery, ApiSecurity, ApiTags } from '@nestjs/swagger';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { InventarioService } from '../inventario/inventario.service';
import { ItemsService } from '../items/items.service';
import { ItemResponseDto, LoteItemResponseDto } from '../items/items.response.dto';
import { TarifasService } from '../items/tarifas.service';
import { CotizacionItemResponseDto } from '../items/tarifas.controller';
import { PreciosService } from '../items/precios.service';
import {
  PrecioItemInternoResponseDto,
  PreciosInternosResponseDto,
  ResolverPreciosInternoRequestDto,
} from '../items/precios.internal.dto';
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
    private readonly precios: PreciosService,
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

  @Get('items/:id/precio')
  @ApiOperation({ summary: 'Resolver el precio de un item por cantidad (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id del item' })
  @ApiQuery({ name: 'empresa_id', required: true })
  @ApiQuery({ name: 'cantidad', required: false, example: 1 })
  @ApiQuery({ name: 'lista', required: false, example: 'mayorista' })
  @ApiQuery({ name: 'segmento', required: false, example: 'mayorista' })
  @ApiOkResponse({ type: PrecioItemInternoResponseDto, description: 'Precio resuelto' })
  async precio(
    @Param('id') id: string,
    @Query('empresa_id') empresaId: string,
    @Query('cantidad') cantidad?: string,
    @Query('lista') lista?: string,
    @Query('segmento') segmento?: string,
  ) {
    const resuelto = await this.precios.resolver(empresaId, id, cantidad ?? 1, {
      lista_codigo: lista,
      segmento_codigo: segmento,
    });
    return {
      item_id: resuelto.item_id,
      cantidad: resuelto.cantidad,
      origen: resuelto.origen,
      precio_unitario_cents: resuelto.precio_unitario_cents,
      precio_total_cents: resuelto.precio_total_cents,
    };
  }

  @Post('items/precios')
  @ApiOperation({ summary: 'Resolver precios por lote (servicio→servicio)' })
  @ApiOkResponse({ type: PreciosInternosResponseDto, description: 'Precios resueltos' })
  async preciosPorLote(@Body() dto: ResolverPreciosInternoRequestDto) {
    const precios = await Promise.all(
      dto.lineas.map(async (linea) => {
        const resuelto = await this.precios.resolver(
          dto.empresa_id,
          linea.item_id,
          linea.cantidad,
          {
            lista_codigo: dto.lista_codigo,
            segmento_codigo: dto.segmento_codigo,
          },
        );
        return {
          clave: linea.clave,
          origen: resuelto.origen,
          precio_unitario_cents: resuelto.precio_unitario_cents,
          precio_total_cents: resuelto.precio_total_cents,
        };
      }),
    );
    return { precios };
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
