import { Injectable } from '@nestjs/common';
import type { PoolClient } from 'pg';
import { PgService, Money, DomainError } from '@core/shared';

export interface Comision {
  id: string;
  order_id: string;
  linea_clave: string;
  empresa_id: string;
  tienda_id: string;
  vendedor_id: string;
  venta_cents: number;
  comision_cents: number;
  monto_vendedor_cents: number;
  monto_empresario_cents: number;
  participacion_porcentaje: number;
  estado: 'acreditada' | 'liquidada' | 'compensada';
  creado_en: string;
}

export interface Pago {
  id: string;
  order_id: string;
  monto_cents: number;
  estado: string;
  metodo: string;
  creado_en: string;
}

/**
 * RN-04: la plataforma cobra su tasa (12 % por defecto) y el neto restante
 * se reparte entre vendedor y empresario según la participación congelada.
 */
@Injectable()
export class ComisionesService {
  private readonly tasa = Number(process.env.COMMISSION_RATE ?? 0.12);

  constructor(private readonly pg: PgService) {}

  tasaComision(): number {
    return this.tasa;
  }

  /** Calcula la comision de una venta (inmutable; los numeros son enteros). */
  calcular(ventaCents: number, participacion = 100, tasa = this.tasa): { comision_cents: number; monto_vendedor_cents: number; monto_empresario_cents: number } {
    if (!Number.isFinite(participacion) || participacion < 0 || participacion > 100) {
      throw new DomainError('PARTICIPACION_INVALIDA', 'La participacion debe estar entre 0 y 100 %.');
    }
    const venta = Money.desdeCentavos(ventaCents);
    const comision = venta.comision(tasa);
    const neto = venta.restar(comision);
    // Money.comision exige una tasa estrictamente menor que 1; los extremos
    // (0 % y 100 %) se resuelven directamente para no rechazar participaciones
    // validas de ADR-25.
    const vendedor = participacion === 100
      ? neto
      : participacion === 0
        ? Money.desdeCentavos(0)
        : neto.comision(participacion / 100);
    return {
      comision_cents: comision.centavos,
      monto_vendedor_cents: vendedor.centavos,
      monto_empresario_cents: neto.restar(vendedor).centavos,
    };
  }

  /** TC-08: registra la comisión de una línea entregada (order.completado). */
  async acreditarLinea(
    datos: { orderId: string; lineaClave: string; empresaId: string; tiendaId: string; vendedorId: string; ventaCents: number; participacion: number },
    client?: PoolClient,
  ): Promise<Comision | null> {
    const { comision_cents, monto_vendedor_cents, monto_empresario_cents } = this.calcular(datos.ventaCents, datos.participacion);
    const existente = client
      ? (await client.query<Comision>(
          `SELECT * FROM commissions.comisiones WHERE order_id = $1 AND linea_clave = $2`,
          [datos.orderId, datos.lineaClave],
        )).rows[0]
      : await this.pg.queryOne<Comision>(
          `SELECT * FROM commissions.comisiones WHERE order_id = $1 AND linea_clave = $2`,
          [datos.orderId, datos.lineaClave],
        );
    if (existente) return null; // idempotencia por orden y línea

    const sql = `INSERT INTO commissions.comisiones
        (id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents, comision_cents,
         monto_vendedor_cents, monto_empresario_cents, participacion_porcentaje, estado)
       VALUES (gen_random_uuid(), $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'acreditada') RETURNING *`;
    const params = [datos.orderId, datos.lineaClave, datos.empresaId, datos.tiendaId, datos.vendedorId,
      datos.ventaCents, comision_cents, monto_vendedor_cents, monto_empresario_cents, datos.participacion];
    const fila = client
      ? (await client.query<Comision>(sql, params)).rows[0]
      : await this.pg.queryOne<Comision>(sql, params);
    if (!fila) throw new DomainError('COMISION_FALLIDA', 'No se pudo acreditar la comision.');
    return fila;
  }

  /**
   * RN-06: devolucion aprobada compensa la comision en la siguiente
   * liquidacion (registro negativo).
   */
  async compensar(orderId: string, motivo: string, client?: PoolClient): Promise<void> {
    const comisiones = client
      ? (await client.query<Comision>(
          `SELECT * FROM commissions.comisiones WHERE order_id = $1`,
          [orderId],
        )).rows
      : await this.pg.query<Comision>(
          `SELECT * FROM commissions.comisiones WHERE order_id = $1`,
          [orderId],
        );
    const ejecutar = async (sql: string, params: unknown[]): Promise<void> => {
      if (client) await client.query(sql, params);
      else await this.pg.query(sql, params);
    };
    for (const comision of comisiones) {
      if (comision.estado === 'compensada') continue;
      await ejecutar(
        `INSERT INTO commissions.compensaciones_devoluciones
          (id, comision_id, order_id, vendedor_id, empresa_id, monto_cents, monto_empresario_cents, motivo, creado_en)
         VALUES (gen_random_uuid(), $1,$2,$3,$4,$5,$6,$7,NOW())`,
        [comision.id, orderId, comision.vendedor_id, comision.empresa_id,
         -comision.monto_vendedor_cents, -comision.monto_empresario_cents, motivo],
      );
      await ejecutar(`UPDATE commissions.comisiones SET estado = 'compensada' WHERE id = $1`, [comision.id]);
    }
  }

  /** Pago simulado del MVP (payment.procesado queda registrado aqui, "Pagos"). */
  async registrarPago(orderId: string, montoCents: number, client?: PoolClient): Promise<Pago> {
    const existente = client
      ? (await client.query<Pago>(
          `SELECT id, order_id, monto_cents, estado, metodo, creado_en FROM commissions.pagos WHERE order_id = $1`,
          [orderId],
        )).rows[0]
      : await this.pg.queryOne<Pago>(
          `SELECT id, order_id, monto_cents, estado, metodo, creado_en FROM commissions.pagos WHERE order_id = $1`,
          [orderId],
        );
    if (existente) return existente;
    const sql = `INSERT INTO commissions.pagos (id, order_id, monto_cents, estado, metodo)
       VALUES (gen_random_uuid(), $1, $2, 'procesado', 'simulado')
       RETURNING id, order_id, monto_cents, estado, metodo, creado_en`;
    const fila = client
      ? (await client.query<Pago>(sql, [orderId, montoCents])).rows[0]
      : await this.pg.queryOne<Pago>(sql, [orderId, montoCents]);
    if (!fila) throw new DomainError('PAGO_FALLIDO', 'No se pudo registrar el pago.');
    return fila;
  }

  async comisionesDe(vendedorId: string): Promise<Comision[]> {
    return this.pg.query<Comision>(
      `SELECT id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents,
              comision_cents, monto_vendedor_cents, monto_empresario_cents,
              participacion_porcentaje, estado, creado_en
       FROM commissions.comisiones WHERE vendedor_id = $1
       ORDER BY creado_en DESC LIMIT 200`,
      [vendedorId],
    );
  }

  async comisionesPorOrden(orderId: string): Promise<Comision[]> {
    return this.pg.query<Comision>(
      `SELECT id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents,
              comision_cents, monto_vendedor_cents, monto_empresario_cents,
              participacion_porcentaje, estado, creado_en
       FROM commissions.comisiones WHERE order_id = $1`,
      [orderId],
    );
  }
}
