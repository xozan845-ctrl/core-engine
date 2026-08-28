import { Injectable, OnModuleDestroy, OnModuleInit, Optional } from '@nestjs/common';
import { randomUUID } from 'crypto';
import { PgService } from '../db/pg.service';
import { RabbitService } from '../rabbitmq/rabbit.service';
import { EventoBus, NombreEvento } from '../events/contracts';
import { Logger } from '../logging/logger';

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
    const evento: EventoBus = {
      event_id,
      tipo,
      ocurrido_en: new Date().toISOString(),
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
    const pendientes = await this.pg.query<{ event_id: string; payload: string }>(
      `SELECT event_id, payload FROM ${this.tabla}
       WHERE estado = 'pendiente' AND creado_en > NOW() - INTERVAL '30 minutes'
       ORDER BY creado_en ASC LIMIT 50
       FOR UPDATE SKIP LOCKED`,
    );
    for (const fila of pendientes) {
      try {
        const evento = this.deserializar(fila.payload);
        const ok = await this.rabbit.publicar(evento);
        if (ok) {
          await this.pg.query(
            `UPDATE ${this.tabla} SET estado = 'publicado', publicado_en = NOW()
             WHERE event_id = $1 AND estado = 'pendiente'`,
            [fila.event_id],
          );
        }
      } catch (err) {
        this.logger.error({
          msg: 'Fallo al publicar evento outbox',
          event_id: fila.event_id,
          err: err instanceof Error ? err.message : String(err),
        });
      }
    }
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