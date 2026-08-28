import { Controller, Get, Param, Query, UseGuards } from '@nestjs/common';
import { ClaveInternaGuard, Logger } from '@core/shared';
import { TributacionService } from './tributacion/tributacion.service';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna.
 * Nunca expuestos por el gateway; usados por identity (situacion fiscal del
 * vendedor) y por reportes/paneles del resto del sistema.
 */
@Controller('internal/finance')
@UseGuards(ClaveInternaGuard)
export class InternoController {
  private readonly logger = Logger.create('finance.interno');

  constructor(private readonly tributacion: TributacionService) {}

  /** Situacion fiscal de un usuario (vendedor/admin) para identity. */
  @Get('sujetos/:usuarioId')
  async sujeto(@Param('usuarioId') usuarioId: string) {
    return this.tributacion.sujetoFiscalDe(usuarioId);
  }

  /** Declaraciones del periodo para reportes de la bodega. */
  @Get('declaraciones')
  async declaraciones(
    @Query('tipo') tipo?: string,
    @Query('periodo_inicio') periodoInicio?: string,
    @Query('estado') estado?: string,
  ) {
    return this.tributacion.declaraciones({ tipo, periodo_inicio: periodoInicio, estado });
  }
}