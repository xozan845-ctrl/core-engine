import { Controller, Get, Param, Query, UseGuards } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiQuery, ApiTags } from '@nestjs/swagger';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { ProductosService } from '../productos/productos.service';
import { InventarioService } from '../inventario/inventario.service';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna.
 */
@ApiTags('Catálogo (interno)')
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(
    private readonly productos: ProductosService,
    private readonly inventario: InventarioService,
  ) {}

  @Get('productos/lote')
  @ApiOperation({ summary: 'Lote de productos por SKUs (servicio→servicio)' })
  @ApiQuery({ name: 'skus', required: false, description: 'SKUs separados por coma' })
  async lote(@Query('skus') skus?: string) {
    const lista = (skus ?? '')
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
    return this.productos.lotePorSkus(lista);
  }

  @Get('productos/sku/:sku')
  @ApiOperation({ summary: 'Producto por SKU (servicio→servicio)' })
  @ApiParam({ name: 'sku', description: 'SKU del producto' })
  async porSku(@Param('sku') sku: string) {
    const producto = await this.productos.encontrarPorSku(sku);
    if (!producto) throw new NotFoundError('Producto', sku);
    return producto;
  }

  @Get('productos/:id')
  @ApiOperation({ summary: 'Producto por id (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id del producto' })
  async porId(@Param('id') id: string) {
    const producto = await this.productos.encontrarPorId(id);
    if (!producto) throw new NotFoundError('Producto', id);
    return producto;
  }

  @Get('inventario/resumen')
  @ApiOperation({ summary: 'Resumen de inventario (servicio→servicio)' })
  async resumen() {
    return this.productos.resumenInventario();
  }

  @Get('inventario/lineas')
  @ApiOperation({ summary: 'Líneas de inventario (servicio→servicio)' })
  async lineas() {
    return this.inventario.listar();
  }
}
