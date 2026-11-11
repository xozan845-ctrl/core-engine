import { Body, Controller, Delete, Get, Param, Post, Query, UseGuards } from '@nestjs/common';
import {
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiQuery,
  ApiSecurity,
  ApiTags,
} from '@nestjs/swagger';
import { ClaveInternaGuard, Logger } from '@core/shared';
import { ComercioService } from './comercio/comercio.service';
import { RegistrarCreditoInternoRequestDto } from './comercio/comercio.dtos';
import { CreditoOrdenResponseDto } from './comercio/comercio.response.dto';
import { TributacionService } from './tributacion/tributacion.service';
import {
  DeclaracionResponseDto,
  SujetoTributarioResponseDto,
} from './tributacion/tributacion.response.dto';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna.
 * Nunca expuestos por el gateway; usados por identity (situacion fiscal del
 * vendedor) y por reportes/paneles del resto del sistema.
 */
@ApiTags('Finanzas (interno)')
@ApiSecurity('internalKey')
@Controller('internal/finance')
@UseGuards(ClaveInternaGuard)
export class InternoController {
  private readonly logger = Logger.create('finance.interno');

  constructor(
    private readonly tributacion: TributacionService,
    private readonly comercio: ComercioService,
  ) {}

  /** Situacion fiscal de un usuario (vendedor/admin) para identity. */
  @Get('sujetos/:usuarioId')
  @ApiOperation({ summary: 'Situación fiscal de un usuario (para identity)' })
  @ApiParam({ name: 'usuarioId', description: 'Id del usuario' })
  @ApiOkResponse({ type: SujetoTributarioResponseDto, description: 'Sujeto fiscal del usuario' })
  async sujeto(@Param('usuarioId') usuarioId: string) {
    return this.tributacion.sujetoFiscalDe(usuarioId);
  }

  /** Venta a crédito: orders registra la cuenta por cobrar antes de crear la orden. */
  @Post('creditos')
  @ApiOperation({ summary: 'Registrar una venta a crédito desde orders' })
  @ApiCreatedResponse({ type: CreditoOrdenResponseDto, description: 'Crédito registrado' })
  async registrarCredito(@Body() dto: RegistrarCreditoInternoRequestDto) {
    return this.comercio.registrarCreditoOrden(dto);
  }

  /** Compensación: la orden no llegó a crearse. */
  @Delete('creditos/orden/:orderId')
  @ApiOperation({ summary: 'Anular el crédito ligado a una orden no creada' })
  @ApiParam({ name: 'orderId', format: 'uuid' })
  @ApiOkResponse({ description: 'Crédito anulado' })
  async anularCredito(@Param('orderId') orderId: string) {
    const ok = await this.comercio.anularCreditoOrden(orderId);
    return { ok };
  }

  /** Declaraciones del periodo para reportes de la bodega. */
  @Get('declaraciones')
  @ApiOperation({ summary: 'Declaraciones del periodo (reportes)' })
  @ApiQuery({ name: 'tipo', required: false })
  @ApiQuery({ name: 'periodo_inicio', required: false })
  @ApiQuery({ name: 'estado', required: false })
  @ApiOkResponse({ type: [DeclaracionResponseDto], description: 'Declaraciones del periodo' })
  async declaraciones(
    @Query('tipo') tipo?: string,
    @Query('periodo_inicio') periodoInicio?: string,
    @Query('estado') estado?: string,
  ) {
    return this.tributacion.declaraciones({ tipo, periodo_inicio: periodoInicio, estado });
  }
}
