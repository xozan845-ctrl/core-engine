import { Controller, Get, Query, Headers, Post, Body } from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiHeader, ApiOkResponse, ApiOperation, ApiTags } from '@nestjs/swagger';
import { ForbiddenError, ROLES, UnauthorizedError } from '@core/shared';
import { InteligenciaService } from '../services/inteligencia.service';
import { FiltrosInteligenciaDto } from '../models/dto/filtros.request.dto';
import { MapaCalorResponseDto } from '../models/dto/mapa-calor.response.dto';
import { RegistrarVentaRequestDto } from '../models/dto/registrar-venta.request.dto';
import {
  CoberturaZonaResponseDto,
  PaginaDemandaResponseDto,
  PaginaRendimientoResponseDto,
  RegistrarVentaResponseDto,
  ResumenInteligenciaResponseDto,
  TendenciaResponseDto,
} from '../models/dto/inteligencia.response.dto';

@ApiTags('Inteligencia — Métricas')
@ApiBearerAuth()
@Controller('api/v1/inteligencia')
export class MetricasController {
  constructor(private readonly inteligencia: InteligenciaService) {}

  @Get('mapa-calor')
  @ApiOperation({ summary: 'Mapa de calor de ventas (admin)' })
  @ApiOkResponse({ type: MapaCalorResponseDto, description: 'Puntos del mapa de calor' })
  async getMapaCalor(
    @Query() filtros: FiltrosInteligenciaDto
  ): Promise<MapaCalorResponseDto> {
    return this.inteligencia.obtenerMapaCalor(filtros);
  }

  @Get('me/mapa-calor')
  @ApiOperation({ summary: 'Mi mapa de calor (vendedor)' })
  @ApiHeader({ name: 'x-user-personal', required: true })
  @ApiOkResponse({ type: MapaCalorResponseDto, description: 'Puntos del mapa de calor del vendedor' })
  async getMiMapaCalor(
    @Headers('x-user-personal') userId: string,
    @Query() filtros: FiltrosInteligenciaDto
  ): Promise<MapaCalorResponseDto> {
    const vendedorId = this.exigirPersonal(userId);
    return this.inteligencia.obtenerMapaCalor({ ...filtros, vendedor_id: vendedorId });
  }

  // ── Rendimiento ───────────────────────────────────────────────────────────

  @Get('rendimiento')
  @ApiOperation({ summary: 'Rendimiento de vendedores (admin)' })
  @ApiOkResponse({ type: PaginaRendimientoResponseDto, description: 'Rendimiento por vendedor' })
  async getRendimiento(@Query() filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number }) {
    return this.inteligencia.obtenerRendimientoVendedores(filtros);
  }

  @Get('me/rendimiento')
  @ApiOperation({ summary: 'Mi rendimiento como vendedor' })
  @ApiHeader({ name: 'x-user-personal', required: true })
  @ApiOkResponse({ type: PaginaRendimientoResponseDto, description: 'Rendimiento del vendedor' })
  async getMiRendimiento(
    @Headers('x-user-personal') userId: string,
    @Query() filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number }
  ) {
    const vendedorId = this.exigirPersonal(userId);
    return this.inteligencia.obtenerRendimientoVendedores({ ...filtros, vendedor_id: vendedorId });
  }

  // ── Otras metricas (Admin) ────────────────────────────────────────────────

  @Get('cobertura')
  @ApiOperation({ summary: 'Cobertura por zona (admin)' })
  @ApiOkResponse({ type: [CoberturaZonaResponseDto], description: 'Cobertura por celda' })
  async getCobertura(@Query() filtros: any) {
    return this.inteligencia.obtenerCoberturaZona(filtros);
  }

  @Get('demanda')
  @ApiOperation({ summary: 'Demanda de productos (admin)' })
  @ApiOkResponse({ type: PaginaDemandaResponseDto, description: 'Demanda por producto' })
  async getDemanda(@Query() filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number }) {
    return this.inteligencia.obtenerDemandaProductos(filtros);
  }

  @Get('tendencias')
  @ApiOperation({ summary: 'Tendencias (admin)' })
  @ApiOkResponse({ type: [TendenciaResponseDto], description: 'Tendencias temporales' })
  async getTendencias(@Query() filtros: FiltrosInteligenciaDto & { fecha_inicio?: string; fecha_fin?: string }) {
    return this.inteligencia.obtenerTendencias(filtros);
  }

  @Get('resumen')
  @ApiOperation({ summary: 'Resumen de inteligencia de mercado (admin)' })
  @ApiOkResponse({ type: ResumenInteligenciaResponseDto, description: 'Resumen ejecutivo' })
  async getResumen(@Query() filtros: FiltrosInteligenciaDto) {
    return this.inteligencia.obtenerResumen(filtros);
  }

  // ── Ingesta manual / Frontend ─────────────────────────────────────────────

  @Post('ventas/registrar')
  @ApiOperation({ summary: 'Registrar una venta en inteligencia (ingesta manual)' })
  @ApiCreatedResponse({ type: RegistrarVentaResponseDto, description: 'Venta registrada' })
  async registrarVenta(
    @Body() data: RegistrarVentaRequestDto,
    @Headers('x-user-personal') personal: string | undefined,
    @Headers('x-user-rol') rol: string | undefined,
  ) {
    // Un vendedor solo registra sus propias ventas: el claim del token manda
    // sobre el cuerpo. Los roles administrativos pueden indicar el vendedor.
    if (rol === ROLES.VENDEDOR) {
      if (!personal) {
        throw new ForbiddenError(
          'El usuario vendedor no tiene ficha de personal vinculada (x-user-personal).',
        );
      }
      data.vendedor_id = personal;
    }
    await this.inteligencia.registrarVenta(data, data.evento_id);
    return { codigo: 'OK', mensaje: 'Venta registrada en inteligencia' };
  }

  /** El endpoint `/me/*` nunca debe ejecutarse sin una ficha de personal. */
  private exigirPersonal(personal: string | undefined): string {
    if (!personal) {
      throw new UnauthorizedError(
        'La ruta /me requiere una ficha de personal vinculada (x-user-personal).',
      );
    }
    return personal;
  }
}
