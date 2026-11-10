import { Body, Controller, Get, Param, Post, Query, UseGuards } from '@nestjs/common';
import { ApiOkResponse, ApiOperation, ApiParam, ApiProperty, ApiQuery, ApiSecurity, ApiTags } from '@nestjs/swagger';
import { IsInt, IsUUID, Max, Min } from 'class-validator';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { OfertasService } from '../ofertas/ofertas.service';
import { OfertaResponseDto } from '../ofertas/ofertas.response.dto';
import { TiendasService } from '../tiendas/tiendas.service';

export class AsignarVendedorRequestDto {
  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  usuario_id: string;

  @ApiProperty({ minimum: 0, maximum: 100 })
  @IsInt()
  @Min(0)
  @Max(100)
  participacion: number;
}

export class DesasignarVendedorRequestDto {
  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  usuario_id: string;
}

/**
 * Endpoints internos (servicio -> servicio) protegidos con clave interna.
 */
@ApiTags('Tiendas (interno)')
@ApiSecurity('internalKey')
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(
    private readonly ofertas: OfertasService,
    private readonly tiendas: TiendasService,
  ) {}

  /** Lote de ofertas por ids (enriquecimiento de precios en el checkout). */
  @Get('ofertas')
  @ApiOperation({ summary: 'Lote de ofertas por ids (servicio→servicio)' })
  @ApiQuery({ name: 'ids', required: false, description: 'Ids de oferta separados por coma' })
  @ApiOkResponse({ type: [OfertaResponseDto], description: 'Ofertas encontradas' })
  async porIds(@Query('ids') idsRaw?: string) {
    const ids = (idsRaw ?? '').split(',').map((s) => s.trim()).filter(Boolean);
    return this.ofertas.porIds(ids);
  }

  /** Guard de integridad: catálogo no puede mutar variantes con ofertas vivas. */
  @Get('ofertas/producto/:productoId/activas')
  @ApiOperation({ summary: 'Contar ofertas activas de un producto (servicio→servicio)' })
  @ApiParam({ name: 'productoId', description: 'Id del producto o variante' })
  @ApiOkResponse({
    description: 'Total de ofertas activas',
    schema: { type: 'object', properties: { total: { type: 'number' } } },
  })
  async activas(@Param('productoId') productoId: string): Promise<{ total: number }> {
    return { total: await this.ofertas.contarActivasPorProducto(productoId) };
  }

  @Get('tiendas')
  @ApiOperation({ summary: 'Tiendas por lote de ids (servicio→servicio)' })
  @ApiQuery({ name: 'ids', required: false })
  @ApiOkResponse({ description: 'Tiendas encontradas', schema: { type: 'object', additionalProperties: true } })
  async tiendasPorIds(@Query('ids') idsRaw?: string) {
    const ids = (idsRaw ?? '').split(',').map((s) => s.trim()).filter(Boolean);
    return { tiendas: await this.tiendas.porIds(ids) };
  }

  @Get('tiendas/empresa/:empresaId')
  @ApiOperation({ summary: 'Tiendas de una empresa (servicio→servicio)' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiOkResponse({ description: 'Tiendas de la empresa', schema: { type: 'object', additionalProperties: true } })
  async tiendasDeEmpresa(@Param('empresaId') empresaId: string) {
    return { tiendas: await this.tiendas.deEmpresa(empresaId) };
  }

  @Get('tiendas/vendedor/:vendedorId')
  @ApiOperation({ summary: 'Tienda asignada a un vendedor (servicio→servicio)' })
  @ApiParam({ name: 'vendedorId', format: 'uuid' })
  @ApiOkResponse({ description: 'Tienda del vendedor o null', schema: { type: 'object', additionalProperties: true } })
  async tiendaDeVendedor(@Param('vendedorId') vendedorId: string) {
    return { tienda: await this.tiendas.deVendedor(vendedorId) };
  }

  @Get('tiendas/:id')
  @ApiOperation({ summary: 'Tienda por id (servicio→servicio)' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ description: 'Tienda encontrada', schema: { type: 'object', additionalProperties: true } })
  async tienda(@Param('id') id: string) {
    const tienda = await this.tiendas.encontrarPorId(id);
    if (!tienda) throw new NotFoundError('Tienda', id);
    return tienda;
  }

  @Post('tiendas/:id/asignar-vendedor')
  @ApiOperation({ summary: 'Asignar vendedor a una tienda (servicio→servicio)' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ description: 'Vendedor asignado', schema: { type: 'object', additionalProperties: true } })
  async asignarVendedor(
    @Param('id') id: string,
    @Body() dto: AsignarVendedorRequestDto,
  ): Promise<{ ok: true }> {
    const tienda = await this.tiendas.encontrarPorId(id);
    if (!tienda) throw new NotFoundError('Tienda', id);
    await this.tiendas.asignarVendedor(id, tienda.empresa_id, dto.usuario_id, dto.participacion);
    return { ok: true };
  }

  @Post('tiendas/:id/desasignar-vendedor')
  @ApiOperation({ summary: 'Compensar asignación de vendedor (servicio→servicio)' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ description: 'Vendedor desasignado', schema: { type: 'object', additionalProperties: true } })
  async desasignarVendedor(
    @Param('id') id: string,
    @Body() dto: DesasignarVendedorRequestDto,
  ): Promise<{ ok: true }> {
    await this.tiendas.desasignarVendedor(id, dto.usuario_id);
    return { ok: true };
  }
}
