import { Injectable } from '@nestjs/common';
import { PgService, Money, DomainError } from '@core/shared';

export interface Comision {
  id: string;
  order_id: string;
  vendedor_id: string;
  venta_cents: number;
  comision_cents: number;
  monto_vendedor_cents: number;
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
 * RN-04: comision = precio de venta x tasa (12 % por defecto); el vendedor
 * recibe precio - comision. Montos en centavos (A02: sin punto flotante).
 */
@Injectable()
export class ComisionesService {
  private readonly tasa = Number(process.env.COMMISSION_RATE ?? 0.12);

  constructor(private readonly pg: PgService) {}

  tasaComision(): number {
    return this.tasa;
  }

  /** Calcula la comision de una venta (inmutable; los numeros son enteros). */
  calcular(ventaCents: number, tasa = this.tasa): { comision_cents: number; monto_vendedor_cents: number } {
    const venta = Money.desdeCentavos(ventaCents);
    const comision = venta.comision(tasa);
    return {
      comision_cents: comision.centavos,
      monto_vendedor_cents: venta.restar(comision).centavos,
    };
  }

  /** TC-08: registra la comision de una orden entregada (order.completado). */
  async acreditar(orderId: string, vendedorId: string, ventaCents: number): Promise<Comision | null> {
    const { comision_cents, monto_vendedor_cents } = this.calcular(ventaCents);
    const existente = await this.pg.queryOne<Comision>(
      `SELECT id, order_id, vendedor_id, venta_cents, comision_cents, monto_vendedor_cents, estado, creado_en
       FROM commissions.comisiones WHERE order_id = $1`,
      [orderId],
    );
    if (existente) return null; // idempotencia por order_id

    const fila = await this.pg.queryOne<Comision>(
      `INSERT INTO commissions.comisiones
        (id, order_id, vendedor_id, venta_cents, comision_cents, monto_vendedor_cents, estado)
       VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, 'acreditada')
       RETURNING id, order_id, vendedor_id, venta_cents, comision_cents, monto_vendedor_cents, estado, creado_en`,
      [orderId, vendedorId, ventaCents, comision_cents, monto_vendedor_cents],
    );
    if (!fila) throw new DomainError('COMISION_FALLIDA', 'No se pudo acreditar la comision.');
    return fila;
  }

  /**
   * RN-06: devolucion aprobada compensa la comision en la siguiente
   * liquidacion (registro negativo).
   */
  async compensar(orderId: string, motivo: string): Promise<void> {
    const comision = await this.pg.queryOne<Comision>(
      `SELECT id, order_id, vendedor_id, venta_cents, comision_cents, monto_vendedor_cents, estado, creado_en
       FROM commissions.comisiones WHERE order_id = $1`,
      [orderId],
    );
    if (!comision) return; // sin comision acreditada: nada que compensar
    if (comision.estado === 'compensada') return;

    await this.pg.query(
      `INSERT INTO commissions.compensaciones_devoluciones
        (id, comision_id, order_id, vendedor_id, monto_cents, motivo, creado_en)
       VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW())`,
      [comision.id, orderId, comision.vendedor_id, -comision.comision_cents, motivo],
    );
    await this.pg.query(
      `UPDATE commissions.comisiones SET estado = 'compensada' WHERE id = $1`,
      [comision.id],
    );
  }

  /** Pago simulado del MVP (payment.procesado queda registrado aqui, "Pagos"). */
  async registrarPago(orderId: string, montoCents: number): Promise<Pago> {
    const existente = await this.pg.queryOne<Pago>(
      `SELECT id, order_id, monto_cents, estado, metodo, creado_en FROM commissions.pagos WHERE order_id = $1`,
      [orderId],
    );
    if (existente) return existente;
    const fila = await this.pg.queryOne<Pago>(
      `INSERT INTO commissions.pagos (id, order_id, monto_cents, estado, metodo)
       VALUES (gen_random_uuid(), $1, $2, 'procesado', 'simulado')
       RETURNING id, order_id, monto_cents, estado, metodo, creado_en`,
      [orderId, montoCents],
    );
    if (!fila) throw new DomainError('PAGO_FALLIDO', 'No se pudo registrar el pago.');
    return fila;
  }

  async comisionesDe(vendedorId: string): Promise<Comision[]> {
    return this.pg.query<Comision>(
      `SELECT id, order_id, vendedor_id, venta_cents, comision_cents, monto_vendedor_cents, estado, creado_en
       FROM commissions.comisiones WHERE vendedor_id = $1
       ORDER BY creado_en DESC LIMIT 200`,
      [vendedorId],
    );
  }

  async comisionPorOrden(orderId: string): Promise<Comision | null> {
    return this.pg.queryOne<Comision>(
      `SELECT id, order_id, vendedor_id, venta_cents, comision_cents, monto_vendedor_cents, estado, creado_en
       FROM commissions.comisiones WHERE order_id = $1`,
      [orderId],
    );
  }
}