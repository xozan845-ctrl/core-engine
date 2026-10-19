import { ApiProperty } from '@nestjs/swagger';

// ── Rendimiento de vendedores ─────────────────────────────────────────────

export class RendimientoVendedorResponseDto {
  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'Total de ventas', example: 42 })
  total_ventas: number;

  @ApiProperty({ description: 'Monto total vendido en centavos', example: 4830000 })
  total_monto_cents: number;

  @ApiProperty({ description: 'Ticket promedio en centavos', example: 115000 })
  ticket_promedio_cents: number;

  @ApiProperty({ description: 'Pedidos entregados', example: 40 })
  pedidos_entregados: number;

  @ApiProperty({ description: 'Tasa de efectividad (0–1)', example: 0.95 })
  tasa_efectividad: number;

  @ApiProperty({ description: 'Última actualización (ISO 8601 UTC)', format: 'date-time' })
  actualizado_en: string;
}

export class PaginaRendimientoResponseDto {
  @ApiProperty({ description: 'Rendimiento por vendedor', type: () => [RendimientoVendedorResponseDto] })
  data: RendimientoVendedorResponseDto[];

  @ApiProperty({ description: 'Cursor de la siguiente página', required: false, nullable: true })
  next_cursor: string | null;
}

// ── Cobertura geográfica ──────────────────────────────────────────────────

export class CoberturaZonaResponseDto {
  @ApiProperty({ description: 'Latitud de la celda', example: 12.1364 })
  lat_cell: number;

  @ApiProperty({ description: 'Longitud de la celda', example: -86.2514 })
  lng_cell: number;

  @ApiProperty({ description: 'Ventas en la celda', example: 18 })
  total_ventas: number;

  @ApiProperty({ description: 'Monto vendido en centavos', example: 2070000 })
  total_monto_cents: number;

  @ApiProperty({ description: 'Vendedores activos en la celda', example: 5 })
  total_vendedores: number;

  @ApiProperty({ description: 'Última actualización (ISO 8601 UTC)', format: 'date-time' })
  actualizado_en: string;

  @ApiProperty({ description: 'Bounding box en GeoJSON', example: '{"type":"Polygon","coordinates":[]}' })
  geojson: string;
}

// ── Demanda de productos ──────────────────────────────────────────────────

export class DemandaProductoResponseDto {
  @ApiProperty({ description: 'SKU del producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Nombre del producto' })
  nombre: string;

  @ApiProperty({ description: 'Unidades vendidas', example: 120 })
  total_vendido: number;

  @ApiProperty({ description: 'Monto vendido en centavos', example: 13800000 })
  total_monto_cents: number;

  @ApiProperty({ description: 'Stock actual', example: 10 })
  stock_actual: number;

  @ApiProperty({ description: 'Última venta (ISO 8601 UTC)', format: 'date-time', required: false, nullable: true })
  ultima_venta_en: string | null;

  @ApiProperty({ description: 'Última actualización (ISO 8601 UTC)', format: 'date-time' })
  actualizado_en: string;
}

export class PaginaDemandaResponseDto {
  @ApiProperty({ description: 'Demanda por producto', type: () => [DemandaProductoResponseDto] })
  data: DemandaProductoResponseDto[];

  @ApiProperty({ description: 'Cursor de la siguiente página', required: false, nullable: true })
  next_cursor: string | null;
}

// ── Tendencias ────────────────────────────────────────────────────────────

export class TendenciaResponseDto {
  @ApiProperty({ description: 'Fecha de la tendencia', format: 'date' })
  fecha: string;

  @ApiProperty({ description: 'SKU del producto', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'Unidades vendidas', example: 12 })
  total_ventas: number;

  @ApiProperty({ description: 'Monto vendido en centavos', example: 1380000 })
  total_monto_cents: number;

  @ApiProperty({ description: 'Día de la semana', example: 3 })
  dia_semana: number;
}

// ── Resumen ejecutivo ─────────────────────────────────────────────────────

export class ResumenInteligenciaResponseDto {
  @ApiProperty({ description: 'Total de pedidos', example: 320 })
  total_pedidos: number;

  @ApiProperty({ description: 'Ingresos totales en centavos', example: 36800000 })
  ingresos_totales_cents: number;

  @ApiProperty({ description: 'Vendedores activos', example: 20 })
  vendedores_activos: number;

  @ApiProperty({ description: 'Fecha de cálculo (ISO 8601 UTC)', format: 'date-time' })
  calculado_en: string;
}

// ── Ingesta manual ────────────────────────────────────────────────────────

export class RegistrarVentaResponseDto {
  @ApiProperty({ description: 'Código de resultado', example: 'OK' })
  codigo: string;

  @ApiProperty({ description: 'Mensaje de resultado', example: 'Venta registrada en inteligencia' })
  mensaje: string;
}

// ── Predicción ────────────────────────────────────────────────────────────

export class ForecastDemandaResponseDto {
  @ApiProperty({ description: 'SKU analizado', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Días proyectados', example: 7 })
  dias_proyectados: number;

  @ApiProperty({ description: 'Ventas proyectadas', example: 84 })
  ventas_proyectadas: number;

  @ApiProperty({ description: 'Límite inferior del intervalo', example: 60 })
  intervalo_inferior: number;

  @ApiProperty({ description: 'Límite superior del intervalo', example: 108 })
  intervalo_superior: number;

  @ApiProperty({ description: 'Media diaria base', example: 12 })
  avg_diario_base: number;

  @ApiProperty({ description: 'Desviación estándar diaria base', example: 3.2 })
  std_diario_base: number;

  @ApiProperty({ description: 'Fecha de cómputo de features', required: false, nullable: true, format: 'date-time' })
  features_computadas_en: string | null;

  @ApiProperty({ description: 'Metodología aplicada', example: 'media_movil_ponderada_7d' })
  metodologia: string;
}

export class AnomaliaResponseDto {
  @ApiProperty({ description: 'Id de la orden', format: 'uuid' })
  order_id: string;

  @ApiProperty({ description: 'SKU del producto' })
  sku: string;

  @ApiProperty({ description: 'Id del vendedor', format: 'uuid' })
  vendedor_id: string;

  @ApiProperty({ description: 'Monto de la venta en centavos', example: 115000 })
  monto_cents: number;

  @ApiProperty({ description: 'Latitud', required: false, nullable: true })
  lat: number | null;

  @ApiProperty({ description: 'Longitud', required: false, nullable: true })
  lng: number | null;

  @ApiProperty({ description: 'Fecha de la venta (ISO 8601 UTC)', format: 'date-time' })
  ocurrido_en: string;

  @ApiProperty({ description: 'Media histórica del monto', example: 115000 })
  media_monto: number;

  @ApiProperty({ description: 'Desviación estándar histórica', example: 15000 })
  std_monto: number;

  @ApiProperty({ description: 'Z-score de la anomalía', example: 3.4 })
  z_score: number;
}

export class AnomaliasResponseDto {
  @ApiProperty({ description: 'Total de anomalías', example: 3 })
  total: number;

  @ApiProperty({ description: 'Anomalías detectadas', type: () => [AnomaliaResponseDto] })
  anomalias: AnomaliaResponseDto[];
}

export class ScoreVendedorResponseDto {
  @ApiProperty({ description: 'Id del vendedor', required: false, format: 'uuid' })
  vendedor_id?: string;

  @ApiProperty({ description: 'Tasa de efectividad', required: false, nullable: true })
  tasa_efectividad?: number | null;

  @ApiProperty({ description: 'Score de riesgo de churn', required: false })
  churn_risk_score?: number;

  @ApiProperty({ description: 'Nivel de riesgo', required: false, enum: ['ALTO', 'MEDIO', 'BAJO'] })
  churn_risk_nivel?: string;

  @ApiProperty({ description: 'Fecha de cómputo de features', required: false, format: 'date-time' })
  features_computadas_en?: string;

  @ApiProperty({ description: 'Mensaje cuando no hay datos computados', required: false })
  mensaje?: string;
}

export class ErrorFrecuenteResponseDto {
  @ApiProperty({ description: 'Tipo de evento', example: 'order.created' })
  event_tipo: string;

  @ApiProperty({ description: 'Error detectado', example: 'campo requerido' })
  error: string;

  @ApiProperty({ description: 'Cantidad de ocurrencias', example: 4 })
  total: number;
}

export class CalidadDatosResponseDto {
  @ApiProperty({ description: 'Ventana analizada en horas', example: 24 })
  ventana_horas: number;

  @ApiProperty({ description: 'Eventos procesados', example: 1200 })
  eventos_procesados: number;

  @ApiProperty({ description: 'Eventos inválidos', example: 12 })
  eventos_invalidos: number;

  @ApiProperty({ description: 'Tasa de rechazo (%)', example: 0.99 })
  tasa_rechazo_pct: number;

  @ApiProperty({ description: 'Errores más frecuentes', type: () => [ErrorFrecuenteResponseDto] })
  errores_frecuentes: ErrorFrecuenteResponseDto[];
}
