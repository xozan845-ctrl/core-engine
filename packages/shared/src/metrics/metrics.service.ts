import { Injectable, OnModuleInit, Optional } from '@nestjs/common';
import { collectDefaultMetrics, Histogram, Counter, Registry } from 'prom-client';
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

  async texto(): Promise<string> {
    return await this.registro.metrics();
  }

  salud(): MetricaSalud {
    return { servicio: this.servicio, uptime_seg: Math.round((Date.now() - this.inicio) / 1000) };
  }
}