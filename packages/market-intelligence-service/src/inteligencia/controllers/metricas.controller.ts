import { Controller, Get, Query, Headers, Post, Body } from '@nestjs/common';
import { ApiBody, ApiHeader, ApiOperation, ApiTags } from '@nestjs/swagger';
import { InteligenciaService } from '../services/inteligencia.service';
import { FiltrosInteligenciaDto } from '../models/dto/filtros.request.dto';
import { MapaCalorResponseDto } from '../models/dto/mapa-calor.response.dto';

@ApiTags('Inteligencia — Métricas')
@Controller('api/v1/inteligencia')
export class MetricasController {
  constructor(private readonly inteligencia: InteligenciaService) {}

  @Get('mapa-calor')
  @ApiOperation({ summary: 'Mapa de calor de ventas (admin)' })
  async getMapaCalor(
    @Query() filtros: FiltrosInteligenciaDto
  ): Promise<MapaCalorResponseDto> {
    return this.inteligencia.obtenerMapaCalor(filtros);
  }

  @Get('me/mapa-calor')
  @ApiOperation({ summary: 'Mi mapa de calor (vendedor)' })
  @ApiHeader({ name: 'x-user-personal', required: false })
  async getMiMapaCalor(
    @Headers('x-user-personal') userId: string,
    @Query() filtros: FiltrosInteligenciaDto
  ): Promise<MapaCalorResponseDto> {
    const f = { ...filtros, vendedor_id: userId };
    return this.inteligencia.obtenerMapaCalor(f);
  }

  // ── Rendimiento ───────────────────────────────────────────────────────────

  @Get('rendimiento')
  @ApiOperation({ summary: 'Rendimiento de vendedores (admin)' })
  async getRendimiento(@Query() filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number }) {
    return this.inteligencia.obtenerRendimientoVendedores(filtros);
  }

  @Get('me/rendimiento')
  @ApiOperation({ summary: 'Mi rendimiento como vendedor' })
  @ApiHeader({ name: 'x-user-personal', required: false })
  async getMiRendimiento(
    @Headers('x-user-personal') userId: string,
    @Query() filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number }
  ) {
    return this.inteligencia.obtenerRendimientoVendedores({ ...filtros, vendedor_id: userId });
  }

  // ── Otras metricas (Admin) ────────────────────────────────────────────────

  @Get('cobertura')
  @ApiOperation({ summary: 'Cobertura por zona (admin)' })
  async getCobertura(@Query() filtros: any) {
    return this.inteligencia.obtenerCoberturaZona(filtros);
  }

  @Get('demanda')
  @ApiOperation({ summary: 'Demanda de productos (admin)' })
  async getDemanda(@Query() filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number }) {
    return this.inteligencia.obtenerDemandaProductos(filtros);
  }

  @Get('tendencias')
  @ApiOperation({ summary: 'Tendencias (admin)' })
  async getTendencias(@Query() filtros: FiltrosInteligenciaDto & { fecha_inicio?: string; fecha_fin?: string }) {
    return this.inteligencia.obtenerTendencias(filtros);
  }

  @Get('resumen')
  @ApiOperation({ summary: 'Resumen de inteligencia de mercado (admin)' })
  async getResumen(@Query() filtros: FiltrosInteligenciaDto) {
    return this.inteligencia.obtenerResumen(filtros);
  }

  // ── Ingesta manual / Frontend ─────────────────────────────────────────────

  @Post('ventas/registrar')
  @ApiOperation({ summary: 'Registrar una venta en inteligencia (ingesta manual)' })
  @ApiBody({ schema: { type: 'object' } })
  async registrarVenta(@Body() data: any) {
    // Aquí el gateway ya validó que el rol pueda acceder, y el validation pipe
    // debería validar el payload real (omitido el DTO estricto por simplicidad MVP)
    await this.inteligencia.registrarVenta(data);
    return { codigo: 'OK', mensaje: 'Venta registrada en inteligencia' };
  }
}
