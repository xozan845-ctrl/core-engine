import { Injectable, OnModuleInit } from '@nestjs/common';
import * as cron from 'node-cron';
import {
  PgService,
  Logger,
} from '@core/shared';

export interface Liquidacion {
  id: string;
  vendedor_id: string;
  periodo_inicio: string;
  periodo_fin: string;
  monto_cents: number;
  estado: 'aprobada' | 'pagada';
  creado_en: string;
}

export interface Periodo {
  inicio: string;
  fin: string;
}

/**
 * RN-07: las comisiones se liquidan quincenalmente (dias 1 y 15) tomando
 * ordenes en estado Entregada (las comisiones solo se acreditan cuando la
 * orden llega a Entregada, ver pedidos.consumer). El corte consolida el
 * libro de pagos del vendedor por periodo.
 */
@Injectable()
export class LiquidacionesService implements OnModuleInit {
  private readonly logger = Logger.create('commissions.liquidacion');

  constructor(private readonly pg: PgService) {}

  onModuleInit(): void {
    // dias 1 y 15 a las 00:05 (zona America/Managua): cierra el periodo previo
    cron.schedule('5 0 1,15 * *', () => {
      this.cerrarPeriodoAnterior(new Date()).catch((err) =>
        this.logger.error({ msg: 'Fallo el corte quincenal', err: err.message }),
      );
    });
    this.logger.info({ msg: 'Planificador de liquidacion quincenal activo (dias 1 y 15)' });
  }

  /** Periodo quincenal al que pertenece una fecha (RN-07: cortes dias 1 y 15). */
  periodoDe(fecha: Date = new Date()): Periodo {
    const anio = fecha.getFullYear();
    const mes = fecha.getMonth();
    const dia = fecha.getDate();
    if (dia <= 15) {
      return {
        inicio: this.fmt(new Date(anio, mes, 1)),
        fin: this.fmt(new Date(anio, mes, 15)),
      };
    }
    return {
      inicio: this.fmt(new Date(anio, mes, 16)),
      fin: this.fmt(new Date(anio, mes + 1, 0)),
    };
  }

  /**
   * Periodo que cierra cada corte (RN-07): el del dia 1 cierra el tramo previo
   * del mes anterior (16-fin); el del dia 15 cierra la 1.ª quincena vigente.
   */
  periodoACerrar(fecha: Date = new Date()): Periodo {
    const anio = fecha.getFullYear();
    const mes = fecha.getMonth();
    const dia = fecha.getDate();
    if (dia === 1) {
      const previo = new Date(anio, mes, 0);
      return {
        inicio: this.fmt(new Date(previo.getFullYear(), previo.getMonth(), 16)),
        fin: this.fmt(previo),
      };
    }
    if (dia <= 15) {
      return {
        inicio: this.fmt(new Date(anio, mes, 1)),
        fin: this.fmt(new Date(anio, mes, 15)),
      };
    }
    return {
      inicio: this.fmt(new Date(anio, mes, 16)),
      fin: this.fmt(new Date(anio, mes + 1, 0)),
    };
  }

  /** Fecha local YYYY-MM-DD (sin desfase UTC). */
  private fmt(fecha: Date): string {
    const anio = fecha.getFullYear();
    const mes = String(fecha.getMonth() + 1).padStart(2, '0');
    const dia = String(fecha.getDate()).padStart(2, '0');
    return `${anio}-${mes}-${dia}`;
  }

  /** Cierra un periodo: consolida comisiones acreditadas y compensaciones. */
  async cerrarPeriodo(periodo: Periodo): Promise<Liquidacion[]> {
    const { inicio, fin } = periodo;

    // 1. marcar como liquidadas las comisiones acreditadas del periodo
    const filas = await this.pg.query<{ id: string; vendedor_id: string; comision_cents: number }>(
      `UPDATE commissions.comisiones SET estado = 'liquidada'
       WHERE estado = 'acreditada' AND creado_en >= $1::date AND creado_en <= $2::date + INTERVAL '1 day' - INTERVAL '1 second'
       RETURNING id, vendedor_id, comision_cents`,
      [inicio, fin],
    );
    if (filas.length === 0) return [];

    // 2. consolidar por vendedor
    const porVendedor = new Map<string, number>();
    for (const f of filas) {
      porVendedor.set(f.vendedor_id, (porVendedor.get(f.vendedor_id) ?? 0) + f.comision_cents);
    }

    // 3. compensaciones del periodo (RN-06: devoluciones)
    const compensaciones = await this.pg.query<{ vendedor_id: string; total: number }>(
      `SELECT vendedor_id, COALESCE(SUM(monto_cents), 0)::int AS total
       FROM commissions.compensaciones_devoluciones
       WHERE creado_en >= $1::date AND creado_en <= $2::date + INTERVAL '1 day' - INTERVAL '1 second'
       GROUP BY vendedor_id`,
      [inicio, fin],
    );
    for (const c of compensaciones) {
      porVendedor.set(c.vendedor_id, (porVendedor.get(c.vendedor_id) ?? 0) + c.total);
    }

    // 4. crear liquidaciones aprobadas
    const liquidaciones: Liquidacion[] = [];
    for (const [vendedorId, monto] of porVendedor) {
      if (monto === 0) continue;
      const fila = await this.pg.queryOne<Liquidacion>(
        `INSERT INTO commissions.liquidaciones
          (id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado)
         VALUES (gen_random_uuid(), $1, $2, $3, $4, 'aprobada')
         ON CONFLICT (vendedor_id, periodo_inicio) DO NOTHING
         RETURNING id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en`,
        [vendedorId, inicio, fin, monto],
      );
      if (fila) liquidaciones.push(fila);
    }

    this.logger.info({
      msg: 'Corte quincenal ejecutado',
      periodo: `${inicio}..${fin}`,
      vendedores: liquidaciones.length,
    });
    return liquidaciones;
  }

  async cerrarPeriodoAnterior(fecha = new Date()): Promise<Liquidacion[]> {
    return this.cerrarPeriodo(this.periodoACerrar(fecha));
  }

  async deVendedor(vendedorId: string): Promise<Liquidacion[]> {
    return this.pg.query<Liquidacion>(
      `SELECT id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en
       FROM commissions.liquidaciones WHERE vendedor_id = $1
       ORDER BY periodo_inicio DESC LIMIT 100`,
      [vendedorId],
    );
  }

  /** Pago de la liquidacion (libro de pagos, quincena pactada). */
  async marcarPagada(liquidacionId: string): Promise<Liquidacion | null> {
    return this.pg.queryOne<Liquidacion>(
      `UPDATE commissions.liquidaciones SET estado = 'pagada'
       WHERE id = $1 RETURNING id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en`,
      [liquidacionId],
    );
  }
}