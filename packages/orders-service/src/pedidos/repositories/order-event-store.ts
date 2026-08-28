import { Injectable } from '@nestjs/common';
import { PgService } from '@core/shared';
import { OrderStatusUpdatedEvent } from '../events/order-events';

export interface EventoDeOrden {
  event_id: string;
  aggregate_id: string;
  aggregate_type: string;
  tipo: string;
  payload: Record<string, unknown>;
  metadata: Record<string, unknown>;
  version: number;
  creado_en: string;
}

/**
 * OrderEventStore: historial append-only de eventos (carpeta repositories del
 * capitulo 3.2). Ningun evento se actualiza ni se borra; es la fuente de verdad.
 */
@Injectable()
export class OrderEventStore {
  constructor(private readonly pg: PgService) {}

  async append(
    orderId: string,
    tipo: string,
    payload: Record<string, unknown>,
  ): Promise<EventoDeOrden> {
    const fila = await this.pg.queryOne<EventoDeOrden>(
      `INSERT INTO orders.eventos (aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en)
       SELECT $1, 'Order', $2, $3::jsonb, '{}'::jsonb, COALESCE(MAX(version), 0) + 1, NOW()
       FROM orders.eventos WHERE aggregate_id = $1
       RETURNING event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en`,
      [orderId, tipo, JSON.stringify(payload)],
    );
    if (!fila) throw new Error('No se pudo anexar evento.');
    return fila;
  }

  async appendEnTransaccion(
    client: import('pg').PoolClient,
    orderId: string,
    tipo: string,
    payload: Record<string, unknown>,
  ): Promise<EventoDeOrden> {
    const r = await client.query<EventoDeOrden>(
      `INSERT INTO orders.eventos (aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en)
       SELECT $1, 'Order', $2, $3::jsonb, '{}'::jsonb, COALESCE(MAX(version), 0) + 1, NOW()
       FROM orders.eventos WHERE aggregate_id = $1
       RETURNING event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en`,
      [orderId, tipo, JSON.stringify(payload)],
    );
    return r.rows[0];
  }

  /** Historia completa de una orden (para replay y reconstruccion). */
  async historiaDe(orderId: string): Promise<EventoDeOrden[]> {
    return this.pg.query<EventoDeOrden>(
      `SELECT event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en
       FROM orders.eventos WHERE aggregate_id = $1 ORDER BY version ASC`,
      [orderId],
    );
  }

  async ultimoEventoDe(orderId: string): Promise<EventoDeOrden | null> {
    return this.pg.queryOne<EventoDeOrden>(
      `SELECT event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en
       FROM orders.eventos WHERE aggregate_id = $1 ORDER BY version DESC LIMIT 1`,
      [orderId],
    );
  }

  async actualizarEstadoActualizado(orderId: string): Promise<void> {
    // mantenido por la proyeccion; aqui solo auditoria del ultimo estado
    const ultimo = await this.ultimoEventoDe(orderId);
    if (ultimo?.tipo === 'OrderStatusUpdatedEvent') {
      const datos = ultimo.payload as unknown as OrderStatusUpdatedEvent;
      void datos;
    }
  }
}