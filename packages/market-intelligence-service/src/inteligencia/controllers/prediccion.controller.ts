import { Controller, Get, Param, Query, Headers } from '@nestjs/common';
import { ApiTags, ApiOperation, ApiBearerAuth } from '@nestjs/swagger';
import { InteligenciaRepository } from '../repositories/inteligencia.repository';

/**
 * Endpoints de Predictive Analytics.
 * La autorización por rol se aplica en el API Gateway (gateway-auth.middleware.ts):
 *   - /api/v1/inteligencia/* → solo ADMIN
 *   - /api/v1/inteligencia/me/* → ADMIN o VENDEDOR
 * Los servicios confían en los headers de contexto inyectados por el gateway
 * (x-user-id, x-user-rol). No se usa guarda local para evitar dependencias de
 * DI extra (Reflector) innecesarias dado el modelo de seguridad del proyecto.
 */
@ApiTags('Inteligencia — Predicción')
@ApiBearerAuth()
@Controller('api/v1/inteligencia')
export class PrediccionController {
  constructor(private readonly repo: InteligenciaRepository) {}

  /**
   * GET /api/v1/inteligencia/forecast/demanda?sku=SKU-TEST&dias=7
   * Proyección de demanda futura basada en media móvil ponderada 7d.
   * Acceso: solo ADMIN (el gateway lo restringe).
   */
  @Get('forecast/demanda')
  @ApiOperation({ summary: 'Forecast de demanda por SKU (media móvil 7d)' })
  async forecastDemanda(
    @Query('sku') sku: string,
    @Query('dias') diasStr?: string,
  ) {
    const dias = Math.min(Math.max(parseInt(diasStr ?? '7', 10) || 7, 1), 90);
    return this.repo.obtenerForecastDemanda(sku, dias);
  }

  /**
   * GET /api/v1/inteligencia/anomalias?sku=&vendedor_id=&limite=
   * Ventas estadísticamente anómalas (Z-score > 2.5 en los últimos 30d).
   * Acceso: solo ADMIN (el gateway lo restringe).
   */
  @Get('anomalias')
  @ApiOperation({ summary: 'Detección de ventas anómalas por Z-score' })
  async anomalias(
    @Query('sku') sku?: string,
    @Query('vendedor_id') vendedorId?: string,
    @Query('limite') limiteStr?: string,
  ) {
    const limite = Math.min(parseInt(limiteStr ?? '50', 10) || 50, 200);
    const resultados = await this.repo.obtenerAnomalias({
      sku,
      vendedor_id: vendedorId,
      limite,
    });
    return { total: resultados.length, anomalias: resultados };
  }

  /**
   * GET /api/v1/inteligencia/vendedores/:id/score
   * Score de riesgo de churn y métricas predictivas de un vendedor.
   * Acceso: solo ADMIN (el gateway lo restringe).
   */
  @Get('vendedores/:id/score')
  @ApiOperation({ summary: 'Score predictivo de un vendedor (churn, efectividad)' })
  async scoreVendedor(@Param('id') vendedorId: string) {
    const score = await this.repo.obtenerScoreVendedor(vendedorId);
    return score ?? {
      vendedor_id: vendedorId,
      mensaje: 'Sin datos computados aún. Las features se calculan cada 5 minutos.',
    };
  }

  /**
   * GET /api/v1/inteligencia/me/score
   * El vendedor autenticado consulta su propio score predictivo.
   * El gateway inyecta x-user-id con el UUID del usuario autenticado.
   * Acceso: ADMIN o VENDEDOR (el gateway lo restringe).
   */
  @Get('me/score')
  @ApiOperation({ summary: 'Mi score predictivo como vendedor' })
  async miScore(@Headers('x-user-id') userId: string) {
    if (!userId) {
      return { mensaje: 'x-user-id header requerido (inyectado por el gateway)' };
    }
    const score = await this.repo.obtenerScoreVendedor(userId);
    return score ?? { vendedor_id: userId, mensaje: 'Sin datos computados aún.' };
  }

  /**
   * GET /api/v1/inteligencia/calidad?horas=24
   * Dashboard de calidad del pipeline: tasa de rechazo, errores frecuentes.
   * Acceso: solo ADMIN (el gateway lo restringe).
   */
  @Get('calidad')
  @ApiOperation({ summary: 'Dashboard de calidad de datos del pipeline (últimas N horas)' })
  async calidadDatos(@Query('horas') horasStr?: string) {
    const horas = Math.min(parseInt(horasStr ?? '24', 10) || 24, 720);
    return this.repo.obtenerCalidadDatos(horas);
  }
}
