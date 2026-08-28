import { Injectable } from '@nestjs/common';
import { PgService } from '@core/shared';
import type { PoolClient } from 'pg';

/**
 * Proyeccion CQRS de metricas mensuales (doc 8.8): materializa los eventos del
 * bus en agregados por mes para el tablero financiero. Upserts aditivos e
 * idempotentes por (periodo, tipo); la deduplicacion total la garantiza el
 * guard de event_id del consumer.
 */
@Injectable()
export class MetricasService {
  constructor(private readonly pg: PgService) {}

  async sumarMetrica(
    client: PoolClient,
    tipo:
      | 'gmv'
      | 'pedidos'
      | 'entregados'
      | 'devoluciones'
      | 'ingresos_comisiones'
      | 'carritos_iniciados'
      | 'checkouts_completados'
      | 'cortes_esperados'
      | 'liquidaciones_a_tiempo',
    montoCents: number,
    cantidad: number,
  ): Promise<void> {
    await client.query(
      `INSERT INTO finance.metricas_mensuales (periodo, tipo, monto_cents, cantidad, actualizado_en)
       VALUES (date_trunc('month', NOW())::date, $1, $2, $3, NOW())
       ON CONFLICT (periodo, tipo) DO UPDATE SET
         monto_cents  = finance.metricas_mensuales.monto_cents + EXCLUDED.monto_cents,
         cantidad     = finance.metricas_mensuales.cantidad + EXCLUDED.cantidad,
         actualizado_en = NOW()`,
      [tipo, montoCents, cantidad],
    );
  }

  async incrementarVendedor(
    client: PoolClient,
    vendedorId: string,
    deltas: { ventas?: number; monto_cents?: number; comision_cents?: number; devoluciones?: number },
  ): Promise<void> {
    await client.query(
      `INSERT INTO finance.metricas_vendedores
        (vendedor_id, periodo, ventas, monto_cents, comision_cents, devoluciones,
         primera_venta_en, ultima_venta_en, actualizado_en)
       VALUES ($1, date_trunc('month', NOW())::date, $2, $3, $4, $5,
               CASE WHEN $2 > 0 THEN NOW() ELSE NULL END, NOW(), NOW())
       ON CONFLICT (vendedor_id, periodo) DO UPDATE SET
         ventas          = finance.metricas_vendedores.ventas + EXCLUDED.ventas,
         monto_cents     = finance.metricas_vendedores.monto_cents + EXCLUDED.monto_cents,
         comision_cents  = finance.metricas_vendedores.comision_cents + EXCLUDED.comision_cents,
         devoluciones    = finance.metricas_vendedores.devoluciones + EXCLUDED.devoluciones,
         primera_venta_en = COALESCE(finance.metricas_vendedores.primera_venta_en, EXCLUDED.primera_venta_en),
         ultima_venta_en = EXCLUDED.ultima_venta_en,
         actualizado_en  = NOW()`,
      [
        vendedorId,
        deltas.ventas ?? 0,
        deltas.monto_cents ?? 0,
        deltas.comision_cents ?? 0,
        deltas.devoluciones ?? 0,
      ],
    );
  }
}