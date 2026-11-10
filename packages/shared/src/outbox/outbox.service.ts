import { Injectable, OnModuleDestroy, OnModuleInit, Optional } from '@nestjs/common';
import { randomUUID } from 'crypto';
import { PgService } from '../db/pg.service';
import { RabbitService } from '../rabbitmq/rabbit.service';
import { EventoBus, NombreEvento } from '../events/contracts';
import { Logger, contextoActual } from '../logging/logger';
import { MetricsService } from '../metrics/metrics.service';

export const METRICAS_OUTBOX = {
  pendientes: 'core_engine_outbox_pending_events',
  publicados: 'core_engine_outbox_published_events_total',
  fallos: 'core_engine_outbox_publish_failures_total',
} as const;

/**
 * Outbox (ADR-03): inserta eventos junto con la transaccion de negocio y un
 * poller los publica al bus. La publicacion es idempotente por event_id.
 */
@Injectable()
export class OutboxService implements OnModuleInit, OnModuleDestroy {
  private timer?: NodeJS.Timeout;
  private detenido = false;
  private readonly logger = Logger.create('outbox');
  private readonly tabla: string;

  constructor(
    private readonly pg: PgService,
    private readonly rabbit: RabbitService,
    /** Cada servicio tiene su propia tabla outbox en su schema (sin duelos de pollers). */
    @Optional() tabla?: string,
    @Optional() private readonly metrics?: MetricsService,
  ) {
    this.tabla = tabla ?? process.env.OUTBOX_TABLA ?? 'outbox';
  }

  onModuleInit(): void {
    this.timer = setInterval(() => this.publicarPendientes().catch(() => undefined), 1000);
  }

  onModuleDestroy(): void {
    this.detenido = true;
    if (this.timer) clearInterval(this.timer);
  }

  /**
   * Inserta un evento en la tabla outbox (debe llamarse DENTRO de la misma
   * transaccion que persiste el cambio de negocio).
   */
  async insertarEnTransaccion(
    client: import('pg').PoolClient,
    tipo: NombreEvento,
    data: unknown,
  ): Promise<string> {
    const event_id = randomUUID();
    const ctx = contextoActual();
    const evento: EventoBus = {
      event_id,
      tipo,
      ocurrido_en: new Date().toISOString(),
      ...(ctx?.request_id ? { request_id: ctx.request_id } : {}),
      data,
    };
    await client.query(
      `INSERT INTO ${this.tabla} (event_id, tipo, payload, estado, creado_en)
       VALUES ($1, $2, $3, 'pendiente', NOW())`,
      [event_id, tipo, JSON.stringify(evento)],
    );
    return event_id;
  }

  /** Inserta fuera de transaccion (para eventos informativos). */
  async insertar(tipo: NombreEvento, data: unknown): Promise<string> {
    return this.pg.transaccion(async (client) => this.insertarEnTransaccion(client, tipo, data));
  }

  private async publicarPendientes(): Promise<void> {
    if (this.detenido) return;
    /**
     * El claim y la publicación comparten transacción: `FOR UPDATE SKIP LOCKED`
     * solo evita duplicados mientras el lock sigue vivo. Si se usara `pg.query`
     * (autocommit), dos pollers podrían leer y publicar el mismo evento.
     *
     * La entrega es at-least-once: si el proceso muere después de publicar y
     * antes del COMMIT, el evento vuelve a quedar pendiente y se reenvía. Los
     * consumidores ya son idempotentes por `event_id`.
     *
     * No se filtra por antigüedad: un evento pendiente debe reintentarse aunque
     * el servicio haya estado caído más de 30 minutos.
     */
    await this.pg.transaccion(async (client) => {
      // El gauge refleja el backlog real del servicio en este ciclo; se toma
      // antes del claim para no confundirlo con el límite de 50 por lote.
      const { rows: [conteo] } = await client.query<{ pendientes: number }>(
        `SELECT COUNT(*)::int AS pendientes FROM ${this.tabla} WHERE estado = 'pendiente'`,
      );
      this.metrics?.registrarGauge(METRICAS_OUTBOX.pendientes, conteo?.pendientes ?? 0);

      const { rows: pendientes } = await client.query<{ event_id: string; payload: string }>(
        `SELECT event_id, payload FROM ${this.tabla}
         WHERE estado = 'pendiente'
         ORDER BY creado_en ASC LIMIT 50
         FOR UPDATE SKIP LOCKED`,
      );
      for (const fila of pendientes) {
        try {
          const evento = this.deserializar(fila.payload);
          const ok = await this.rabbit.publicar(evento);
          if (ok) {
            await client.query(
              `UPDATE ${this.tabla} SET estado = 'publicado', publicado_en = NOW()
               WHERE event_id = $1 AND estado = 'pendiente'`,
              [fila.event_id],
            );
            this.metrics?.incrementarContador(METRICAS_OUTBOX.publicados);
          } else {
            this.metrics?.incrementarContador(METRICAS_OUTBOX.fallos);
          }
        } catch (err) {
          this.metrics?.incrementarContador(METRICAS_OUTBOX.fallos);
          this.logger.error({
            msg: 'Fallo al publicar evento outbox',
            event_id: fila.event_id,
            err: err instanceof Error ? err.message : String(err),
          });
        }
      }
    });
  }

  /** Auditoria manual: reenvia eventos publicados (replay de proyecciones). */
  async reenviar(event_id?: string): Promise<number> {
    const where = event_id ? 'WHERE event_id = $1' : "WHERE estado = 'publicado'";
    const params = event_id ? [event_id] : [];
    const filas = await this.pg.query<{ event_id: string; payload: string }>(
      `SELECT event_id, payload FROM ${this.tabla} ${where} ORDER BY creado_en ASC LIMIT 200`,
      params,
    );
    for (const fila of filas) {
      this.rabbit.publicar(this.deserializar(fila.payload)).catch(() => undefined);
    }
    return filas.length;
  }

  /** jsonb o text: pg devuelve jsonb como objeto; JSON.parse solo aplica a string. */
  private deserializar(payload: string | EventoBus): EventoBus {
    if (typeof payload === 'string') return JSON.parse(payload) as EventoBus;
    return payload as EventoBus;
  }
}