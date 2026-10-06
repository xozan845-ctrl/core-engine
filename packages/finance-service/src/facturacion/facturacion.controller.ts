import { Controller, Get, Post, Param, Body, Query } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiProperty, ApiQuery, ApiTags } from '@nestjs/swagger';
import { IsIn, IsOptional, IsString, IsUUID } from 'class-validator';
import { Roles, ROLES, NotFoundError } from '@core/shared';
import { FacturacionService, Comprobante } from './facturacion.service';

export class EmitirComprobanteRequestDto {
  @ApiProperty({ description: 'Tipo de comprobante', enum: ['FACTURA', 'NOTA_CREDITO'] })
  @IsIn(['FACTURA', 'NOTA_CREDITO'])
  tipo: 'FACTURA' | 'NOTA_CREDITO';

  @ApiProperty({ description: 'Id de la orden (UUID)' })
  @IsUUID()
  orden_id: string;

  @ApiProperty({ description: 'Id del cliente', required: false })
  @IsOptional()
  @IsString()
  cliente_id?: string;

  @ApiProperty({ description: 'Razón social', required: false })
  @IsOptional()
  @IsString()
  razon_social?: string;

  @ApiProperty({ description: 'RUC', required: false })
  @IsOptional()
  @IsString()
  ruc?: string;
}

/**
 * GET/POST /api/v1/finanzas/comprobantes — comprobantes fiscales DGI
 * (Ley 822: IVA 15 %; Ley 842: emision en cordobas).
 */
@ApiTags('Finanzas · Facturación')
@Controller('api/v1/finanzas/comprobantes')
@Roles(ROLES.ADMIN)
export class FacturacionController {
  constructor(private readonly facturacion: FacturacionService) {}

  @Get()
  @ApiOperation({ summary: 'Listar comprobantes fiscales' })
  @ApiQuery({ name: 'desde', required: false })
  @ApiQuery({ name: 'hasta', required: false })
  @ApiQuery({ name: 'estado', required: false })
  async listar(
    @Query('desde') desde?: string,
    @Query('hasta') hasta?: string,
    @Query('estado') estado?: string,
  ): Promise<Comprobante[]> {
    return this.facturacion.listar(desde, hasta, estado);
  }

  @Get('series')
  @ApiOperation({ summary: 'Series de comprobantes' })
  async series() {
    return this.facturacion.series();
  }

  @Post()
  @ApiOperation({ summary: 'Emitir un comprobante (factura o nota de crédito)' })
  async emitir(@Body() dto: EmitirComprobanteRequestDto): Promise<Comprobante> {
    return this.facturacion.emitir(dto.tipo, dto.orden_id, {
      cliente_id: dto.cliente_id,
      razon_social: dto.razon_social,
      ruc: dto.ruc,
    });
  }

  @Post(':id/anular')
  @ApiOperation({ summary: 'Anular un comprobante' })
  @ApiParam({ name: 'id', description: 'Id del comprobante' })
  async anular(@Param('id') id: string): Promise<Comprobante> {
    const comprobante = await this.facturacion.anular(id);
    if (!comprobante) throw new NotFoundError('Comprobante', id);
    return comprobante;
  }
}
