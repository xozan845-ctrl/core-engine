import { Controller, Get, Query, UseGuards, Headers, Post, Body } from '@nestjs/common';
import { InteligenciaService } from '../services/inteligencia.service';
import { FiltrosInteligenciaDto } from '../models/dto/filtros.request.dto';
import { MapaCalorResponseDto } from '../models/dto/mapa-calor.response.dto';
import { ROLES, ClaveInternaGuard } from '@core/shared';

@Controller('api/v1/inteligencia')
export class MetricasController {
  constructor(private readonly inteligencia: InteligenciaService) {}

  @Get('mapa-calor')
  async getMapaCalor(
    @Query() filtros: FiltrosInteligenciaDto
  ): Promise<MapaCalorResponseDto> {
    return this.inteligencia.obtenerMapaCalor(filtros);
  }

  @Get('me/mapa-calor')
  async getMiMapaCalor(
    @Headers('x-user-personal') userId: string,
    @Query() filtros: FiltrosInteligenciaDto
  ): Promise<MapaCalorResponseDto> {
    const f = { ...filtros, vendedor_id: userId };
    return this.inteligencia.obtenerMapaCalor(f);
  }

  // ── Rendimiento ───────────────────────────────────────────────────────────

  @Get('rendimiento')
  async getRendimiento(@Query() filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number }) {
    return this.inteligencia.obtenerRendimientoVendedores(filtros);
  }

  @Get('me/rendimiento')
  async getMiRendimiento(
    @Headers('x-user-personal') userId: string,
    @Query() filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number }
  ) {
    return this.inteligencia.obtenerRendimientoVendedores({ ...filtros, vendedor_id: userId });
  }

  // ── Otras metricas (Admin) ────────────────────────────────────────────────

  @Get('cobertura')
  async getCobertura(@Query() filtros: any) {
    return this.inteligencia.obtenerCoberturaZona(filtros);
  }

  @Get('demanda')
  async getDemanda(@Query() filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number }) {
    return this.inteligencia.obtenerDemandaProductos(filtros);
  }

  @Get('tendencias')
  async getTendencias(@Query() filtros: FiltrosInteligenciaDto & { fecha_inicio?: string; fecha_fin?: string }) {
    return this.inteligencia.obtenerTendencias(filtros);
  }

  @Get('resumen')
  async getResumen(@Query() filtros: FiltrosInteligenciaDto) {
    return this.inteligencia.obtenerResumen(filtros);
  }

  // ── Ingesta manual / Frontend ─────────────────────────────────────────────

  @Post('ventas/registrar')
  async registrarVenta(@Body() data: any) {
    // Aquí el gateway ya validó que el rol pueda acceder, y el validation pipe
    // debería validar el payload real (omitido el DTO estricto por simplicidad MVP)
    await this.inteligencia.registrarVenta(data);
    return { codigo: 'OK', mensaje: 'Venta registrada en inteligencia' };
  }
}

