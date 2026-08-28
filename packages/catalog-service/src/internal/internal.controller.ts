import { Controller, Get, Param, Query, UseGuards } from '@nestjs/common';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { ProductosService } from '../productos/productos.service';
import { InventarioService } from '../inventario/inventario.service';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna.
 */
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(
    private readonly productos: ProductosService,
    private readonly inventario: InventarioService,
  ) {}

  @Get('productos/lote')
  async lote(@Query('skus') skus?: string) {
    const lista = (skus ?? '')
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
    return this.productos.lotePorSkus(lista);
  }

  @Get('productos/sku/:sku')
  async porSku(@Param('sku') sku: string) {
    const producto = await this.productos.encontrarPorSku(sku);
    if (!producto) throw new NotFoundError('Producto', sku);
    return producto;
  }

  @Get('productos/:id')
  async porId(@Param('id') id: string) {
    const producto = await this.productos.encontrarPorId(id);
    if (!producto) throw new NotFoundError('Producto', id);
    return producto;
  }

  @Get('inventario/resumen')
  async resumen() {
    return this.productos.resumenInventario();
  }

  @Get('inventario/lineas')
  async lineas() {
    return this.inventario.listar();
  }
}