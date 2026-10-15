import { Injectable, OnModuleInit, Optional } from '@nestjs/common';
import { collectDefaultMetrics, Histogram, Counter, Gauge, Registry } from 'prom-client';
import { NOMBRE_SERVICIOS } from '../constants';

export interface MetricaSalud {
  servicio: string;
  uptime_seg: number;
}

type EtiquetaHttp = 'servicio' | 'metodo' | 'ruta' | 'estado';
type TipoMetricaNegocio = 'counter' | 'gauge';

export const METRICAS_HTTP = {
  peticiones: 'core_engine_http_requests_total',
  latencia: 'core_engine_http_request_duration_seconds',
  erroresServidor: 'core_engine_http_server_errors_total',
  erroresCliente: 'core_engine_http_relevant_client_errors_total',
} as const;

const ETIQUETAS_HTTP: EtiquetaHttp[] = ['servicio', 'metodo', 'ruta', 'estado'];
const NOMBRE_METRICA = /^core_engine_[a-z][a-z0-9]*(?:_[a-z][a-z0-9]*){2,}$/;
const ESTADOS_CLIENTE_RELEVANTES = new Set([401, 403, 409]);
const PATRON_UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const PATRON_ID = /^(?:\d+|[0-9a-f]{24}|[a-z][a-z0-9]*-\d+[a-z0-9-]*)$/i;

/**
 * Reduce URLs crudas a rutas de cardinalidad acotada cuando Express no entrega
 * el patrón declarado. Conserva segmentos estáticos y parámetros `:nombre`.
 */
export function parametrizarRuta(ruta: string): string {
  const sinQuery = (ruta.split(/[?#]/, 1)[0] || '/').replace(/\/{2,}/g, '/');
  const segmentos = sinQuery.split('/').filter(Boolean).map((segmento) => {
    if (segmento.startsWith(':') || segmento === '*') return segmento;
    if (PATRON_UUID.test(segmento) || PATRON_ID.test(segmento)) return ':id';
    return segmento;
  });
  return `/${segmentos.join('/')}`;
}

export function validarNombreMetrica(
  nombre: string,
  tipo: TipoMetricaNegocio,
): void {
  if (!NOMBRE_METRICA.test(nombre)) {
    throw new Error(
      `Métrica inválida: ${nombre}. Usa core_engine_<subsistema>_<nombre>_<unidad>.`,
    );
  }
  if (tipo === 'counter' && !nombre.endsWith('_total')) {
    throw new Error(`Contador inválido: ${nombre}. Debe terminar en _total.`);
  }
  if (tipo === 'gauge' && nombre.endsWith('_total')) {
    throw new Error(`Gauge inválido: ${nombre}. El sufijo _total está reservado a contadores.`);
  }
}

/**
 * Registro Prometheus por servicio + rutas de salud liveness/readiness
 * (doc 5.2: health checks ligeros de liveness y readiness).
 */
@Injectable()
export class MetricsService implements OnModuleInit {
  readonly registro = new Registry();
  private inicio = Date.now();
  private readonly servicio: string;
  private latencia!: Histogram<EtiquetaHttp>;
  private peticiones!: Counter<EtiquetaHttp>;
  private erroresServidor!: Counter<EtiquetaHttp>;
  private erroresCliente!: Counter<EtiquetaHttp>;

  /** Contadores de negocio dinámicos (nombre → instancia). */
  private readonly contadores = new Map<string, Counter<string>>();
  /** Gauges de negocio dinámicos (nombre → instancia). */
  private readonly gauges = new Map<string, Gauge<string>>();

  constructor(@Optional() servicio?: string) {
    this.servicio = servicio ?? process.env.SERVICIO ?? NOMBRE_SERVICIOS.GATEWAY;
  }

  onModuleInit(): void {
    collectDefaultMetrics({ register: this.registro });
    this.latencia = new Histogram({
      name: METRICAS_HTTP.latencia,
      help: 'Duración de peticiones HTTP en segundos',
      buckets: [0.05, 0.1, 0.2, 0.4, 0.8, 1.6, 3.2],
      labelNames: ETIQUETAS_HTTP,
      registers: [this.registro],
    });
    this.peticiones = new Counter({
      name: METRICAS_HTTP.peticiones,
      help: 'Total de peticiones HTTP',
      labelNames: ETIQUETAS_HTTP,
      registers: [this.registro],
    });
    this.erroresServidor = new Counter({
      name: METRICAS_HTTP.erroresServidor,
      help: 'Total de respuestas HTTP 5xx',
      labelNames: ETIQUETAS_HTTP,
      registers: [this.registro],
    });
    this.erroresCliente = new Counter({
      name: METRICAS_HTTP.erroresCliente,
      help: 'Total de respuestas HTTP 401, 403 y 409',
      labelNames: ETIQUETAS_HTTP,
      registers: [this.registro],
    });
  }

  /** Interceptor universal de latencia por peticion. */
  registrarPeticion(
    metodo: string,
    ruta: string,
    status: number,
    duracionMs: number,
  ): void {
    const labels = {
      servicio: this.servicio,
      metodo: metodo.toUpperCase(),
      ruta: parametrizarRuta(ruta),
      estado: Number.isInteger(status) && status >= 100 && status <= 599
        ? String(status)
        : 'desconocido',
    };
    this.latencia.observe(labels, Math.max(0, duracionMs) / 1000);
    this.peticiones.inc(labels);
    if (status >= 500 && status <= 599) this.erroresServidor.inc(labels);
    if (ESTADOS_CLIENTE_RELEVANTES.has(status)) this.erroresCliente.inc(labels);
  }

  /**
   * Incrementa (o crea) un contador de negocio por nombre.
   * Los contadores son acumulativos y monotónicamente crecientes.
   * Uso: this.metrics.incrementarContador('core_engine_intelligence_invalid_events_total')
   */
  incrementarContador(nombre: string, incremento = 1): void {
    validarNombreMetrica(nombre, 'counter');
    if (!Number.isFinite(incremento) || incremento < 0) {
      throw new Error('El incremento de un contador debe ser finito y no negativo.');
    }
    if (!this.contadores.has(nombre)) {
      this.contadores.set(
        nombre,
        new Counter({
          name: nombre,
          help: `Counter de negocio: ${nombre}`,
          registers: [this.registro],
        }),
      );
    }
    this.contadores.get(nombre)!.inc(incremento);
  }

  /**
   * Establece (o crea) un gauge de negocio por nombre.
   * Los gauges pueden subir y bajar (ej: tamaño de cola, eventos pendientes).
   * Uso: this.metrics.registrarGauge('core_engine_intelligence_queue_pending_items', 12)
   */
  registrarGauge(nombre: string, valor: number): void {
    validarNombreMetrica(nombre, 'gauge');
    if (!Number.isFinite(valor)) throw new Error('El valor de un gauge debe ser finito.');
    if (!this.gauges.has(nombre)) {
      this.gauges.set(
        nombre,
        new Gauge({
          name: nombre,
          help: `Gauge de negocio: ${nombre}`,
          registers: [this.registro],
        }),
      );
    }
    this.gauges.get(nombre)!.set(valor);
  }

  async texto(): Promise<string> {
    return await this.registro.metrics();
  }

  salud(): MetricaSalud {
    return { servicio: this.servicio, uptime_seg: Math.round((Date.now() - this.inicio) / 1000) };
  }
}
