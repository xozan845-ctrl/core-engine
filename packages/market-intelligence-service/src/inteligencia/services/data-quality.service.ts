import { Injectable, Logger } from '@nestjs/common';
import Ajv from 'ajv';
import { VentaGeolocalizadaSchema } from '../models/schemas/venta-geo.schema';
import { MetricsService, PgService } from '@core/shared';

export interface ValidationResult {
  isValid: boolean;
  errors: string[];
  completenessScore: number;
  qualityTier: 'HIGH' | 'MEDIUM' | 'LOW';
}

// Nombres de métricas Prometheus (centralizados para evitar typos)
const METRIC = {
  EVENTOS_INVALIDOS: 'intelligence_invalid_events_total',
  COMPLETENESS_HIGH: 'intelligence_data_quality_high_total',
  COMPLETENESS_MED: 'intelligence_data_quality_medium_total',
  COMPLETENESS_LOW: 'intelligence_data_quality_low_total',
} as const;

@Injectable()
export class DataQualityService {
  private readonly logger = new Logger(DataQualityService.name);
  private ajv: Ajv;
  private validateVentaGeo: any;

  constructor(
    private readonly metrics: MetricsService,
    private readonly pg: PgService,
  ) {
    this.ajv = new Ajv({ allErrors: true });
    this.validateVentaGeo = this.ajv.compile(VentaGeolocalizadaSchema);
  }

  validarVentaGeolocalizada(event: any): ValidationResult {
    const isValid = this.validateVentaGeo(event);
    const errors: string[] = isValid
      ? []
      : this.validateVentaGeo.errors?.map((e: any) => `${e.instancePath} ${e.message}`) || [];

    // Validaciones de negocio extra que van más allá del JSON Schema
    if (event.lat && (event.lat < -90 || event.lat > 90)) {
      errors.push('lat fuera de rango válido [-90, 90]');
    }

    if (event.monto_cents !== undefined && event.monto_cents <= 0) {
      errors.push('monto_cents debe ser positivo');
    }

    const isFullyValid = isValid && errors.length === 0;

    if (!isFullyValid) {
      this.logger.warn(`Evento inválido detectado: ${JSON.stringify(errors)}`);
      // Contador Prometheus (sin await: fire-and-forget para no bloquear el pipeline)
      this.metrics.incrementarContador(METRIC.EVENTOS_INVALIDOS);
    }

    // Calcular completitud de datos (Data completeness score)
    const optionalFields = [
      'lat', 'lng', 'precision', 'velocidad', 'rumbo',
      'tipo_actividad', 'resultado_visita', 'distancia_cliente_metros',
      'rango_edad', 'genero',
    ];
    let filledFields = 0;
    for (const field of optionalFields) {
      if (event[field] !== undefined && event[field] !== null && event[field] !== '') {
        filledFields++;
      }
    }
    const completenessScore = filledFields / optionalFields.length;
    let qualityTier: 'HIGH' | 'MEDIUM' | 'LOW' = 'LOW';
    if (completenessScore > 0.9) qualityTier = 'HIGH';
    else if (completenessScore > 0.5) qualityTier = 'MEDIUM';

    // Registrar tier de calidad en Prometheus
    this.metrics.incrementarContador(
      qualityTier === 'HIGH'
        ? METRIC.COMPLETENESS_HIGH
        : qualityTier === 'MEDIUM'
        ? METRIC.COMPLETENESS_MED
        : METRIC.COMPLETENESS_LOW,
    );

    this.logger.debug(`Data completeness: ${(completenessScore * 100).toFixed(0)}% (${qualityTier})`);

    return { isValid: isFullyValid, errors, completenessScore, qualityTier };
  }

  /**
   * Persiste un evento inválido en intelligence.invalid_events para auditoría.
   * Se llama desde VentasConsumer tras detectar un fallo de Data Contract.
   * Es fire-and-forget: un error de inserción no debe bloquear el consumer.
   */
  async registrarEventoInvalido(
    eventId: string | undefined,
    eventTipo: string,
    errores: string[],
    payload: any,
  ): Promise<void> {
    try {
      await this.pg.query(
        `INSERT INTO intelligence.invalid_events (event_id, event_tipo, errores, payload)
         VALUES ($1, $2, $3, $4)`,
        [eventId ?? null, eventTipo, errores, JSON.stringify(payload)],
      );
    } catch (err: any) {
      // No relanzar: un fallo de auditoría no debe parar el pipeline
      this.logger.error(`No se pudo persistir evento inválido: ${err.message}`);
    }
  }
}
