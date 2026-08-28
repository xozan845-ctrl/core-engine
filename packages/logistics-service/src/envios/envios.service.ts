import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import { PgService, DomainError, Logger, Money } from '@core/shared';

export interface Envio {
  id: string;
  guia: string;
  order_id: string;
  estado: string;
  creado_en: string;
}

export interface OrdenExterna {
  id: string;
  cliente_id: string;
  items: { sku: string; cantidad: number; producto_nombre: string }[];
  total: string;
  total_cents: number;
  estado: string;
}

/**
 * Logistica: el sistema logístico automatiza la operacion de la bodega
 * (Tabla 1). Genera la guia de despacho y notifica a las partes.
 */
@Injectable()
export class EnviosService {
  private readonly ordersUrl = process.env.ORDERS_SERVICE_URL ?? 'http://orders-service:3004';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly logger = Logger.create('envios');

  constructor(private readonly pg: PgService) {}

  /** Prepara la guia de despacho cuando el pago fue procesado (payment.procesado). */
  async prepararEnvio(orderId: string, montoCents: number): Promise<Envio> {
    const existente = await this.pg.queryOne<Envio>(
      `SELECT id, guia, order_id, estado, creado_en FROM logistics.envios WHERE order_id = $1`,
      [orderId],
    );
    if (existente) return existente; // idempotencia del consumidor

    const guia = `BGH-${Date.now().toString(36).toUpperCase()}-${randomUUID().slice(0, 6).toUpperCase()}`;
    const fila = await this.pg.queryOne<Envio>(
      `INSERT INTO logistics.envios (id, guia, order_id, monto_cents, estado)
       VALUES ($1, $2, $3, $4, 'en_preparacion')
       RETURNING id, guia, order_id, estado, creado_en`,
      [randomUUID(), guia, orderId, montoCents],
    );
    if (!fila) throw new DomainError('ENVIO_FALLIDO', 'No se pudo preparar el envio.');
    this.logger.info({ msg: 'Guia de despacho generada', order_id: orderId, guia: fila.guia });
    return fila;
  }

  async deOrden(orderId: string): Promise<Envio | null> {
    return this.pg.queryOne<Envio>(
      `SELECT id, guia, order_id, estado, creado_en FROM logistics.envios WHERE order_id = $1`,
      [orderId],
    );
  }

  async listar(): Promise<(Envio & { monto: string })[]> {
    const filas = await this.pg.query<Envio & { monto_cents: number }>(
      `SELECT id, guia, order_id, estado, monto_cents, creado_en FROM logistics.envios
       ORDER BY creado_en DESC LIMIT 200`,
    );
    return filas.map((f) => ({
      id: f.id,
      guia: f.guia,
      order_id: f.order_id,
      estado: f.estado,
      creado_en: f.creado_en,
      monto: Money.desdeCentavos(f.monto_cents).string(),
    }));
  }

  /** PATCH /orders/:id/estado: avanza el ciclo de vida (Tabla 13). */
  async avanzarEstadoOrden(
    orderId: string,
    estadoNuevo: string,
    motivo?: string,
  ): Promise<OrdenExterna> {
    const url = `${this.ordersUrl}/internal/orders/${encodeURIComponent(orderId)}/transicion?estado=${encodeURIComponent(estadoNuevo)}${motivo ? `&motivo=${encodeURIComponent(motivo)}` : ''}`;
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'x-internal-key': this.claveInterna },
    });
    if (!res.ok) {
      const cuerpo = (await res.json().catch(() => ({}))) as { codigo?: string; mensaje?: string };
      throw new DomainError(
        cuerpo.codigo ?? 'TRANSICION_FALLIDA',
        cuerpo.mensaje ?? 'La transicion de estado fue rechazada.',
      );
    }
    const orden = (await res.json()) as OrdenExterna;
    this.logger.info({ msg: 'Estado de orden avanzado', order_id: orderId, estado: estadoNuevo });
    return orden;
  }

  /** Consulta una orden (para notificaciones y guias). */
  async consultarOrden(orderId: string): Promise<OrdenExterna | null> {
    try {
      const res = await fetch(`${this.ordersUrl}/internal/orders/${encodeURIComponent(orderId)}`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) return null;
      return (await res.json()) as OrdenExterna;
    } catch {
      return null;
    }
  }
}