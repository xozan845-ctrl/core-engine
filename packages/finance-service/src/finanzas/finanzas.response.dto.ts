import { ApiProperty } from '@nestjs/swagger';
import { SujetoTributarioResponseDto } from '../tributacion/tributacion.response.dto';

export class SupuestosProyeccionResponseDto {
  @ApiProperty({ example: 12 })
  horizonte_meses: number;

  @ApiProperty({ example: 20 })
  vendedores_iniciales: number;

  @ApiProperty({ example: 5 })
  entrada_vendedores_mes: number;

  @ApiProperty({ example: 0 })
  churn_tasa: number;

  @ApiProperty({ type: [Number], example: [25, 27, 29, 31, 33, 35, 37, 38, 39, 40, 41, 42] })
  pedidos_por_vendedor: number[];

  @ApiProperty({ example: 45000 })
  ticket_promedio_cents: number;

  @ApiProperty({ example: 0.12 })
  comision_tasa: number;

  @ApiProperty({ example: 200000 })
  costos_fijos_cents: number;

  @ApiProperty({ required: false, example: 300000 })
  costos_fijos_desde_mes7_cents?: number;

  @ApiProperty({ example: 5850000 })
  inversion_inicial_cents: number;

  @ApiProperty({ required: false, example: 0.015 })
  tasa_descuento_mensual?: number;
}

export class FilaProyeccionResponseDto {
  @ApiProperty() mes: number;
  @ApiProperty() vendedores: number;
  @ApiProperty() pedidos: number;
  @ApiProperty() gmv_cents: number;
  @ApiProperty() ingresos_cents: number;
  @ApiProperty() costos_cents: number;
  @ApiProperty() flujo_cents: number;
  @ApiProperty() flujo_acumulado_cents: number;
  @ApiProperty() costo_fijo_por_pedido_cents: number;
}

export class TotalesProyeccionResponseDto {
  @ApiProperty() gmv_cents: number;
  @ApiProperty() ingresos_cents: number;
  @ApiProperty() costos_cents: number;
  @ApiProperty() flujo_cents: number;
}

export class EscenarioEquilibrioResponseDto {
  @ApiProperty({ enum: ['bajo', 'medio', 'alto'] }) escenario: string;
  @ApiProperty() costo_variable_por_pedido_cents: number;
  @ApiProperty() contribucion_por_pedido_cents: number;
  @ApiProperty() pedidos_equilibrio: number;
  @ApiProperty() gmv_equilibrio_cents: number;
}

export class CoberturaPorVendedorResponseDto {
  @ApiProperty() pedidos_por_vendedor_mes: number;
  @ApiProperty() comision_por_pedido_cents: number;
  @ApiProperty() comision_bruta_vendedor_cents: number;
  @ApiProperty() contribucion_media_cents: number;
  @ApiProperty() vendedores_para_cubrir_costos: number;
}

export class EconomiaDeEscalaResponseDto {
  @ApiProperty() costo_fijo_por_pedido_inicio_cents: number;
  @ApiProperty() costo_fijo_por_pedido_mes6_cents: number;
  @ApiProperty() costo_fijo_por_pedido_fin_cents: number;
  @ApiProperty() multiplicador_escala: number;
}

export class MargenSeguridadResponseDto {
  @ApiProperty() mes: number;
  @ApiProperty() gmv_cents: number;
  @ApiProperty() margen_seguridad: number;
}

export class IndicadoresProyeccionResponseDto {
  @ApiProperty({ nullable: true }) payback_mes: number | null;
  @ApiProperty() van_cents: number;
  @ApiProperty({ nullable: true }) tir_mensual: number | null;
  @ApiProperty({ nullable: true }) tir_anual: number | null;
  @ApiProperty() tir_no_estable: boolean;
  @ApiProperty() roi: number;
  @ApiProperty() margen_neto: number;
  @ApiProperty() punto_equilibrio_caja_cents: number;
  @ApiProperty() pedidos_equilibrio_caja: number;
  @ApiProperty({ type: () => [EscenarioEquilibrioResponseDto] })
  escenarios_equilibrio: EscenarioEquilibrioResponseDto[];
  @ApiProperty({ type: () => [MargenSeguridadResponseDto] })
  margen_seguridad_por_mes: MargenSeguridadResponseDto[];
  @ApiProperty({ type: CoberturaPorVendedorResponseDto })
  cobertura_por_vendedor: CoberturaPorVendedorResponseDto;
  @ApiProperty({ type: EconomiaDeEscalaResponseDto })
  economia_de_escala: EconomiaDeEscalaResponseDto;
}

export class ProyeccionResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty() nombre: string;
  @ApiProperty() horizonte_meses: number;
  @ApiProperty({ type: SupuestosProyeccionResponseDto }) supuestos: SupuestosProyeccionResponseDto;
  @ApiProperty({ type: () => [FilaProyeccionResponseDto] }) filas: FilaProyeccionResponseDto[];
  @ApiProperty({ type: TotalesProyeccionResponseDto }) totales: TotalesProyeccionResponseDto;
  @ApiProperty() inversion_inicial_cents: number;
  @ApiProperty({ type: IndicadoresProyeccionResponseDto }) indicadores: IndicadoresProyeccionResponseDto;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
}

export class KpisFinancierosResponseDto {
  @ApiProperty() periodo: string;
  @ApiProperty() gmv_cents: number;
  @ApiProperty() gmv: string;
  @ApiProperty() pedidos_pagados: number;
  @ApiProperty() pedidos_entregados: number;
  @ApiProperty() ingresos_comisiones_cents: number;
  @ApiProperty() ingresos_comisiones: string;
  @ApiProperty() devoluciones: number;
  @ApiProperty() vendedores_activos: number;
  @ApiProperty() pedidos_por_vendedor: number;
  @ApiProperty({ nullable: true }) activacion_primeros_15_dias: number | null;
  @ApiProperty() tasa_devoluciones: number;
  @ApiProperty() churn_mensual: number;
  @ApiProperty({ nullable: true }) conversion_carrito_checkout: number | null;
  @ApiProperty({ nullable: true }) liquidacion_a_tiempo: number | null;
}

export class FlujoVariacionGmvResponseDto {
  @ApiProperty() variacion_gmv: number;
  @ApiProperty() flujo_cents: number;
}

export class SensibilidadComisionGmvResponseDto {
  @ApiProperty() tasa_comision: number;
  @ApiProperty({ type: () => [FlujoVariacionGmvResponseDto] })
  flujo_mes12_cents: FlujoVariacionGmvResponseDto[];
}

export class SensibilidadVanResponseDto {
  @ApiProperty() tasa_anual: number;
  @ApiProperty() tasa_mensual: number;
  @ApiProperty() van_cents: number;
  @ApiProperty() relacion_van_inversion: number;
}

export class PaybackPorEscenarioResponseDto {
  @ApiProperty({ enum: ['base', 'pesimista', 'peor_combinado'] }) escenario: string;
  @ApiProperty() descripcion: string;
  @ApiProperty() flujo_acumulado_cents: number;
  @ApiProperty({ nullable: true }) payback_mes: number | null;
}

export class ResumenAnualResponseDto {
  @ApiProperty() anio: number;
  @ApiProperty() gmv_cents: number;
  @ApiProperty() ingresos_cents: number;
  @ApiProperty() costos_cents: number;
  @ApiProperty() flujo_cents: number;
  @ApiProperty() margen_neto: number;
}

export class Acumulado24MesesResponseDto {
  @ApiProperty() gmv_cents: number;
  @ApiProperty() ingresos_cents: number;
  @ApiProperty() costos_cents: number;
  @ApiProperty() flujo_cents: number;
}

export class Mes24ResponseDto {
  @ApiProperty() gmv_cents: number;
  @ApiProperty() incremento_vs_mes12: number;
}

export class PlanBienalResponseDto {
  @ApiProperty() inversion_inicial_cents: number;
  @ApiProperty({ type: () => [ResumenAnualResponseDto] }) anios: ResumenAnualResponseDto[];
  @ApiProperty({ type: () => [FilaProyeccionResponseDto] }) filas: FilaProyeccionResponseDto[];
  @ApiProperty({ type: Acumulado24MesesResponseDto })
  acumulado_24_meses: Acumulado24MesesResponseDto;
  @ApiProperty() roi: number;
  @ApiProperty({ type: Mes24ResponseDto }) mes_24: Mes24ResponseDto;
  @ApiProperty({ type: IndicadoresProyeccionResponseDto }) indicadores: IndicadoresProyeccionResponseDto;
}

export class PuntoEquilibrioResponseDto {
  @ApiProperty() punto_equilibrio_caja_cents: number;
  @ApiProperty() pedidos_equilibrio_caja: number;
  @ApiProperty({ type: () => [EscenarioEquilibrioResponseDto] })
  escenarios_equilibrio: EscenarioEquilibrioResponseDto[];
  @ApiProperty({ type: () => [MargenSeguridadResponseDto] })
  margen_seguridad_por_mes: MargenSeguridadResponseDto[];
}

export class TableroAlertaResponseDto {
  @ApiProperty() capa: string;
  @ApiProperty() frecuencia: string;
  @ApiProperty() indicador: string;
  @ApiProperty({ enum: ['OK', 'ALERTA', 'SIN_DATO'] }) estado: string;
  @ApiProperty() umbral: string;
  @ApiProperty({ nullable: true }) valor: number | null;
  @ApiProperty() accion: string;
  @ApiProperty({ required: false }) detalle?: string;
  @ApiProperty() fecha_esperada: string;
}

export class TableroResponseDto {
  @ApiProperty() periodo: string;
  @ApiProperty({ type: KpisFinancierosResponseDto }) kpis: KpisFinancierosResponseDto;
  @ApiProperty({ type: () => [TableroAlertaResponseDto] })
  alertas: TableroAlertaResponseDto[];
}

export class SensibilidadEstudiosResponseDto {
  @ApiProperty({ type: SupuestosProyeccionResponseDto }) supuestos: SupuestosProyeccionResponseDto;
  @ApiProperty({ type: () => [SensibilidadComisionGmvResponseDto] })
  matriz_comision_gmv: SensibilidadComisionGmvResponseDto[];
  @ApiProperty({ type: () => [SensibilidadVanResponseDto] })
  van_por_tasa: SensibilidadVanResponseDto[];
  @ApiProperty({ type: () => [PaybackPorEscenarioResponseDto] })
  payback_por_escenario: PaybackPorEscenarioResponseDto[];
  @ApiProperty({ type: CoberturaPorVendedorResponseDto })
  cobertura_por_vendedor: CoberturaPorVendedorResponseDto;
}

export class ResumenVendedorResponseDto {
  @ApiProperty({ format: 'uuid' }) vendedor_id: string;
  @ApiProperty() periodo: string;
  @ApiProperty() ventas: number;
  @ApiProperty() monto_cents: number;
  @ApiProperty() comision_cents: number;
  @ApiProperty() devoluciones: number;
}

export class VendedorFiscalResponseDto {
  @ApiProperty({ type: SujetoTributarioResponseDto, nullable: true })
  sujeto_fiscal: SujetoTributarioResponseDto | null;

  @ApiProperty({ type: ResumenVendedorResponseDto, nullable: true })
  resumen_comisiones: ResumenVendedorResponseDto | null;

  @ApiProperty() nota: string;
}
