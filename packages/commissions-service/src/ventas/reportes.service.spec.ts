import { afterEach, describe, expect, it, vi } from 'vitest';
import { ReportesService } from './reportes.service';

function crearServicio() {
  const pg = {
    queryOne: vi.fn(async (sql: string) => {
      if (sql.includes('SUM(venta_cents)')) return { total: 12345 };
      if (sql.includes('SUM(comision_cents)')) return { total: 2345 };
      if (sql.includes('compensaciones_devoluciones')) return { n: 2 };
      if (sql.includes('FROM commissions.pagos')) return { n: 4 };
      return null;
    }),
  };
  const servicio = new ReportesService(pg as never);
  return { servicio, pg };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('ReportesService', () => {
  it('debe construir los KPIs con valores monetarios y conteos del periodo', async () => {
    const { servicio, pg } = crearServicio();
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        if (url.includes('inventario')) {
          return new Response(JSON.stringify({ productos: 12, stock_total: 80, agotados: 3 }), { status: 200 });
        }
        return new Response(JSON.stringify({ vendedores: 5 }), { status: 200 });
      }),
    );

    await expect(servicio.kpis()).resolves.toEqual({
      gmv_mes_cents: 12345,
      gmv_mes: '123.45',
      pedidos_mes: 4,
      comisiones_mes_cents: 2345,
      comisiones_mes: '23.45',
      vendedores_activos: 5,
      productos: 12,
      stock_total: 80,
      agotados: 3,
      devoluciones_mes: 2,
    });
    expect(pg.queryOne).toHaveBeenCalledTimes(4);
  });

  it('debe degradar los KPIs externos a cero cuando inventario o identidad no responden', async () => {
    const { servicio } = crearServicio();
    vi.stubGlobal('fetch', vi.fn(async () => { throw new Error('servicio no disponible'); }));

    await expect(servicio.kpis()).resolves.toMatchObject({
      vendedores_activos: 0,
      productos: 0,
      stock_total: 0,
      agotados: 0,
    });
  });
});
