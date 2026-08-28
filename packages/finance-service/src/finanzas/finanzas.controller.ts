import { Body, Controller, Get, Param, Post, Query } from '@nestjs/common';
import {
  ArrayMinSize,
  IsInt,
  IsNumber,
  IsOptional,
  IsString,
  IsUUID,
  Max,
  Min,
  MinLength,
  ValidateNested,
} from 'class-validator';
import { Type } from 'class-transformer';
import { Roles, ROLES, NotFoundError } from '@core/shared';
import { ProyeccionesService, Proyeccion, SupuestosProyeccion } from './proyecciones.service';
import { KpisService, KpisFinancieros } from './kpis.service';
import { AlertasService } from './alertas.service';

export class SupuestosDto {
  @IsInt()
  @Min(1)
  @Max(60)
  horizonte_meses: number;

  @IsInt()
  @Min(1)
  vendedores_iniciales: number;

  @IsInt()
  @Min(0)
  entrada_vendedores_mes: number;

  @IsNumber()
  @Min(0)
  @Max(1)
  churn_tasa: number;

  @ArrayMinSize(1)
  @IsInt({ each: true })
  @Min(0, { each: true })
  pedidos_por_vendedor: number[];

  @IsInt()
  @Min(1)
  ticket_promedio_cents: number;

  @IsNumber()
  @Min(0)
  @Max(1)
  comision_tasa: number;

  @IsInt()
  @Min(0)
  costos_fijos_cents: number;

  @IsOptional()
  @IsInt()
  @Min(0)
  costos_fijos_desde_mes7_cents?: number;

  @IsInt()
  @Min(0)
  inversion_inicial_cents: number;

  @IsOptional()
  @IsNumber()
  @Min(0)
  tasa_descuento_mensual?: number;
}

export class CrearProyeccionDto {
  @IsString()
  @MinLength(3)
  nombre: string;

  @ValidateNested()
  @Type(() => SupuestosDto)
  supuestos: SupuestosDto;
}

/**
 * GET/POST /api/v1/finanzas/{proyecciones,punto-equilibrio,kpis} — modelo
 * financiero del cap. 8: proyecciones auditables, KPIs y equilibrio.
 */
@Controller('api/v1/finanzas')
@Roles(ROLES.ADMIN)
export class FinanzasController {
  constructor(
    private readonly proyecciones: ProyeccionesService,
    private readonly kpis: KpisService,
    private readonly alertas: AlertasService,
  ) {}

  @Post('proyecciones')
  async crear(@Body() dto: CrearProyeccionDto): Promise<Proyeccion> {
    return this.proyecciones.crear(dto.nombre, dto.supuestos as SupuestosProyeccion);
  }

  @Get('proyecciones')
  async listar() {
    return this.proyecciones.listar();
  }

  @Get('proyecciones/:id')
  async detalle(@Param('id') id: string): Promise<Proyeccion> {
    const proyeccion = await this.proyecciones.obtener(id);
    if (!proyeccion) throw new NotFoundError('Proyeccion', id);
    return proyeccion;
  }

  /** Calculo puntual del punto de equilibrio (tablas 8.12 y 8.11 del informe). */
  @Get('punto-equilibrio')
  async puntoEquilibrio(
    @Query('costos_fijos_cents') costosFijos?: string,
    @Query('ticket_promedio_cents') ticket?: string,
    @Query('comision_tasa') comision?: string,
    @Query('horizonte_meses') horizonte?: string,
  ) {
    const supuestos: SupuestosProyeccion = {
      horizonte_meses: Number(horizonte ?? 12),
      vendedores_iniciales: 1,
      entrada_vendedores_mes: 0,
      churn_tasa: 0,
      pedidos_por_vendedor: Array.from({ length: Number(horizonte ?? 12) }, (_, i) =>
        Math.min(42, 25 + Math.floor(i * 1.6)),
      ),
      ticket_promedio_cents: Number(ticket ?? 45000),
      comision_tasa: Number(comision ?? 0.12),
      costos_fijos_cents: Number(costosFijos ?? 300000),
      inversion_inicial_cents: 0,
    };
    const { filas } = this.proyecciones.calcular(supuestos);
    const indicadores = this.proyecciones.indicadores(filas, supuestos);
    return {
      punto_equilibrio_caja_cents: indicadores.punto_equilibrio_caja_cents,
      pedidos_equilibrio_caja: indicadores.pedidos_equilibrio_caja,
      escenarios_equilibrio: indicadores.escenarios_equilibrio,
      margen_seguridad_por_mes: indicadores.margen_seguridad_por_mes,
    };
  }

  /** KPIs financieros del periodo (tabla 8.8). */
  @Get('kpis')
  async kpisDelMes(@Query('mes') mes?: string): Promise<KpisFinancieros> {
    return this.kpis.kpis(mes);
  }

  /**
   * Estudios del cap. 8: matriz de sensibilidad comision x demanda (8.10),
   * VAN por tasa de descuento 12-30 % (8.11), payback por escenario (8.12)
   * y cobertura por vendedor (8.7). Con los supuestos del informe por defecto.
   */
  @Get('proyecciones/sensibilidad')
  async sensibilidad(): Promise<{
    supuestos: SupuestosProyeccion;
    matriz_comision_gmv: ReturnType<ProyeccionesService['sensibilidadComisionGmv']>;
    van_por_tasa: ReturnType<ProyeccionesService['sensibilidadVan']>;
    payback_por_escenario: ReturnType<ProyeccionesService['paybackPorEscenario']>;
    cobertura_por_vendedor: ReturnType<ProyeccionesService['coberturaPorVendedor']>;
  }> {
    const base: SupuestosProyeccion = {
      horizonte_meses: 12,
      vendedores_iniciales: 20,
      entrada_vendedores_mes: 5,
      churn_tasa: 0,
      pedidos_por_vendedor: [25, 27, 29, 31, 33, 35, 37, 38, 39, 40, 41, 42],
      ticket_promedio_cents: 45000,
      comision_tasa: 0.12,
      costos_fijos_cents: 200000,
      costos_fijos_desde_mes7_cents: 300000,
      inversion_inicial_cents: 5850000,
      tasa_descuento_mensual: 0.015,
    };
    const { filas } = this.proyecciones.calcular(base);
    return {
      supuestos: base,
      matriz_comision_gmv: this.proyecciones.sensibilidadComisionGmv(base),
      van_por_tasa: this.proyecciones.sensibilidadVan(filas, base),
      payback_por_escenario: this.proyecciones.paybackPorEscenario(base),
      cobertura_por_vendedor: this.proyecciones.coberturaPorVendedor(base),
    };
  }

  /** Plan a 24 meses (8.6 + 8.9): cuenta de resultados por anio, ROI, mes 24. */
  @Get('proyecciones/plan-bienal')
  async planBienal(): Promise<ReturnType<ProyeccionesService['planBienal']>> {
    const base: SupuestosProyeccion = {
      horizonte_meses: 12,
      vendedores_iniciales: 20,
      entrada_vendedores_mes: 5,
      churn_tasa: 0,
      pedidos_por_vendedor: [25, 27, 29, 31, 33, 35, 37, 38, 39, 40, 41, 42],
      ticket_promedio_cents: 45000,
      comision_tasa: 0.12,
      costos_fijos_cents: 200000,
      costos_fijos_desde_mes7_cents: 300000,
      inversion_inicial_cents: 5850000,
      tasa_descuento_mensual: 0.015,
    };
    return this.proyecciones.planBienal(base);
  }

  /** Tablero financiero: KPIs + reglas de alerta por capa (tabla 8.10). */
  @Get('tablero')
  async tablero(
    @Query('mes') mes?: string,
    @Query('costo_entrega_por_pedido_cents') costoEntrega?: string,
  ) {
    return this.alertas.tablero({
      mes,
      costo_entrega_por_pedido_cents:
        costoEntrega === undefined ? undefined : Number(costoEntrega),
    });
  }
}