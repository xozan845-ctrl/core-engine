import { Controller, Get, Post, Param, Body, Query } from '@nestjs/common';
import { IsIn, IsOptional, IsString, IsUUID } from 'class-validator';
import { Roles, ROLES, NotFoundError } from '@core/shared';
import { FacturacionService, Comprobante } from './facturacion.service';

export class EmitirComprobanteRequestDto {
  @IsIn(['FACTURA', 'NOTA_CREDITO'])
  tipo: 'FACTURA' | 'NOTA_CREDITO';

  @IsUUID()
  orden_id: string;

  @IsOptional()
  @IsString()
  cliente_id?: string;

  @IsOptional()
  @IsString()
  razon_social?: string;

  @IsOptional()
  @IsString()
  ruc?: string;
}

/**
 * GET/POST /api/v1/finanzas/comprobantes — comprobantes fiscales DGI
 * (Ley 822: IVA 15 %; Ley 842: emision en cordobas).
 */
@Controller('api/v1/finanzas/comprobantes')
@Roles(ROLES.ADMIN)
export class FacturacionController {
  constructor(private readonly facturacion: FacturacionService) {}

  @Get()
  async listar(
    @Query('desde') desde?: string,
    @Query('hasta') hasta?: string,
    @Query('estado') estado?: string,
  ): Promise<Comprobante[]> {
    return this.facturacion.listar(desde, hasta, estado);
  }

  @Get('series')
  async series() {
    return this.facturacion.series();
  }

  @Post()
  async emitir(@Body() dto: EmitirComprobanteRequestDto): Promise<Comprobante> {
    return this.facturacion.emitir(dto.tipo, dto.orden_id, {
      cliente_id: dto.cliente_id,
      razon_social: dto.razon_social,
      ruc: dto.ruc,
    });
  }

  @Post(':id/anular')
  async anular(@Param('id') id: string): Promise<Comprobante> {
    const comprobante = await this.facturacion.anular(id);
    if (!comprobante) throw new NotFoundError('Comprobante', id);
    return comprobante;
  }
}
