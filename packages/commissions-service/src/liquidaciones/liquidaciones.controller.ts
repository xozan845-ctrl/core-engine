import { Controller, Get, Param, Post, Query } from '@nestjs/common';
import { IsIn, IsOptional, IsString } from 'class-validator';
import {
  Roles,
  ROLES,
  UsuarioActual,
  UsuarioContexto,
  NotFoundError,
} from '@core/shared';
import { LiquidacionesService, Liquidacion, Periodo } from './liquidaciones.service';

export class EstadoLiquidacionRequestDto {
  @IsIn(['pagada'])
  estado: string;
}

@Controller('api/v1')
export class LiquidacionesController {
  constructor(private readonly liquidaciones: LiquidacionesService) {}

  /** GET /api/v1/vendedores/me/liquidaciones — cortes del vendedor. */
  @Get('vendedores/me/liquidaciones')
  @Roles(ROLES.VENDEDOR)
  async mias(@UsuarioActual() usuario: UsuarioContexto): Promise<Liquidacion[]> {
    return this.liquidaciones.deVendedor(usuario.user_id);
  }

  /** POST /api/v1/admin/liquidaciones/corte — corte manual (admin, pruebas). */
  @Post('admin/liquidaciones/corte')
  @Roles(ROLES.ADMIN)
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
