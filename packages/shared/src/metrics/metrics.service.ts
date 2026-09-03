import { Injectable, OnModuleInit, Optional } from '@nestjs/common';
import { collectDefaultMetrics, Histogram, Counter, Gauge, Registry } from 'prom-client';
import { NOMBRE_SERVICIOS } from '../constants';

export interface MetricaSalud {
  servicio: string;
  uptime_seg: number;
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
  private latencia: Histogram<string>;
  private peticiones: Counter<string>;

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
      name: 'http_peticion_latencia_ms',
      help: 'Latencia de peticiones HTTP en milisegundos',
      buckets: [50, 100, 200, 400, 800, 1600, 3200],
      labelNames: ['servicio', 'metodo', 'ruta', 'status'],
      registers: [this.registro],
    });
    this.peticiones = new Counter({
      name: 'http_peticiones_total',
      help: 'Total de peticiones HTTP',
      labelNames: ['servicio', 'metodo', 'ruta', 'status'],
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
    const etiquetaRuta = ruta.length > 60 ? `${ruta.slice(0, 57)}...` : ruta;
    this.latencia.observe({ servicio: this.servicio, metodo, ruta: etiquetaRuta, status: String(status) }, duracionMs);
    this.peticiones.inc({ servicio: this.servicio, metodo, ruta: etiquetaRuta, status: String(status) });
  }

  /**
   * Incrementa (o crea) un contador de negocio por nombre.
   * Los contadores son acumulativos y monotónicamente crecientes.
   * Uso: this.metrics.incrementarContador('intelligence_invalid_events_total')
   */
  incrementarContador(nombre: string, incremento = 1): void {
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
   * Uso: this.metrics.registrarGauge('intelligence_feature_compute_duration_ms', 142)
   */
  registrarGauge(nombre: string, valor: number): void {
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