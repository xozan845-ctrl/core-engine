import { Controller, Get, Param, Post, Query } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiProperty, ApiQuery, ApiTags } from '@nestjs/swagger';
import { IsIn } from 'class-validator';
import {
  Roles,
  ROLES,
  UsuarioActual,
  UsuarioContexto,
  NotFoundError,
} from '@core/shared';
import { LiquidacionesService, Liquidacion, Periodo } from './liquidaciones.service';

export class EstadoLiquidacionRequestDto {
  @ApiProperty({ description: 'Estado de la liquidación', enum: ['pagada'] })
  @IsIn(['pagada'])
  estado: string;
}

@ApiTags('Comisiones · Liquidaciones')
@Controller('api/v1')
export class LiquidacionesController {
  constructor(private readonly liquidaciones: LiquidacionesService) {}

  /** GET /api/v1/vendedores/me/liquidaciones — cortes del vendedor. */
  @Get('vendedores/me/liquidaciones')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Cortes/liquidaciones del vendedor autenticado' })
  async mias(@UsuarioActual() usuario: UsuarioContexto): Promise<Liquidacion[]> {
    return this.liquidaciones.deVendedor(usuario.user_id);
  }

  /** POST /api/v1/admin/liquidaciones/corte — corte manual (admin, pruebas). */
  @Post('admin/liquidaciones/corte')
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'Ejecutar un corte de liquidaciones (admin)' })
  @ApiQuery({ name: 'inicio', required: false })
  @ApiQuery({ name: 'fin', required: false })
  async corteManual(
    @Query('inicio') inicio?: string,
    @Query('fin') fin?: string,
  ): Promise<Liquidacion[]> {
    if (inicio && fin) {
      const periodo: Periodo = { inicio, fin };
      return this.liquidaciones.cerrarPeriodo(periodo);
    }
    return this.liquidaciones.cerrarPeriodoAnterior(new Date());
  }

  /** PATCH /api/v1/admin/liquidaciones/:id/pagar — cierre de pago (RN-07). */
  @Post('admin/liquidaciones/:id/pagar')
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'Marcar una liquidación como pagada (RN-07)' })
  @ApiParam({ name: 'id', description: 'Id de la liquidación' })
  @ApiQuery({ name: 'estado', required: false })
  async pagar(
    @Param('id') id: string,
    @Query('estado') estado?: string,
  ): Promise<Liquidacion> {
    void estado;
    const liquidacion = await this.liquidaciones.marcarPagada(id);
    if (!liquidacion) throw new NotFoundError('Liquidacion', id);
    return liquidacion;
  }
}
