import { Injectable } from '@nestjs/common';
import { PgService, Money } from '@core/shared';

export interface ReporteKPI {
  gmv_mes_cents: number;
  gmv_mes: string;
  pedidos_mes: number;
  comisiones_mes_cents: number;
  comisiones_mes: string;
  vendedores_activos: number;
  productos: number;
  stock_total: number;
  agotados: number;
  devoluciones_mes: number;
}

/**
 * H-06 / GET /api/v1/admin/reportes — KPIs de ventas e inventario (Tabla 21).
 * Las metricas derivan de los eventos de negocio (doc 8.8: misma fuente de
 * verdad que el sistema, sin reconciliacion manual).
 */
@Injectable()
export class ReportesService {
  private readonly catalogoUrl = process.env.CATALOG_SERVICE_URL ?? 'http://catalog-service:3002';
  private readonly identidadUrl = process.env.IDENTITY_SERVICE_URL ?? 'http://identity-service:3001';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';

  constructor(private readonly pg: PgService) {}

  async kpis(): Promise<ReporteKPI> {
    const [gmv, comisiones, devoluciones, inventario, vendedores] = await Promise.all([
      this.gmvDelMes(),
      this.comisionesDelMes(),
      this.devolucionesDelMes(),
      this.inventarioResumen(),
      this.contarVendedores(),
    ]);

    return {
      gmv_mes_cents: gmv,
      gmv_mes: Money.desdeCentavos(gmv).string(),
      pedidos_mes: await this.pedidosDelMes(),
      comisiones_mes_cents: comisiones,
      comisiones_mes: Money.desdeCentavos(comisiones).string(),
      vendedores_activos: vendedores,
      productos: inventario.productos,
      stock_total: inventario.stock_total,
      agotados: inventario.agotados,
      devoluciones_mes: devoluciones,
    };
  }

  private async gmvDelMes(): Promise<number> {
    const fila = await this.pg.queryOne<{ total: number }>(
      `SELECT COALESCE(SUM(venta_cents), 0)::int AS total
       FROM commissions.comisiones WHERE creado_en >= date_trunc('month', NOW())`,
    );
    return fila?.total ?? 0;
  }

  private async pedidosDelMes(): Promise<number> {
    const fila = await this.pg.queryOne<{ n: number }>(
      `SELECT COUNT(*)::int AS n FROM commissions.pagos
       WHERE creado_en >= date_trunc('month', NOW())`,
    );
    return fila?.n ?? 0;
  }

  private async comisionesDelMes(): Promise<number> {
    const fila = await this.pg.queryOne<{ total: number }>(
      `SELECT COALESCE(SUM(comision_cents), 0)::int AS total
       FROM commissions.comisiones WHERE creado_en >= date_trunc('month', NOW())`,
    );
    return fila?.total ?? 0;
  }

  private async devolucionesDelMes(): Promise<number> {
    const fila = await this.pg.queryOne<{ n: number }>(
      `SELECT COUNT(*)::int AS n FROM commissions.compensaciones_devoluciones
       WHERE creado_en >= date_trunc('month', NOW())`,
    );
    return fila?.n ?? 0;
  }

  private async inventarioResumen(): Promise<{ productos: number; stock_total: number; agotados: number }> {
    try {
      const res = await fetch(`${this.catalogoUrl}/internal/inventario/resumen`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) return { productos: 0, stock_total: 0, agotados: 0 };
      return (await res.json()) as { productos: number; stock_total: number; agotados: number };
    } catch {
      return { productos: 0, stock_total: 0, agotados: 0 };
    }
  }

  private async contarVendedores(): Promise<number> {
    try {
      const res = await fetch(`${this.identidadUrl}/internal/contar-vendedores`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) return 0;
      const cuerpo = (await res.json()) as { vendedores?: number };
      return cuerpo.vendedores ?? 0;
    } catch {
      return 0;
    }
  }
}