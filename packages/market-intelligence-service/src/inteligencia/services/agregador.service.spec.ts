import type { Mock } from 'vitest';
import { AgregadorService } from './agregador.service';
import { EMPRESA_MERCADO_ID } from '@core/shared';

/**
 * ADR-27: las métricas de producto se agregan por empresa. Las ventas
 * geolocalizadas que no identifican empresa usan el bucket de mercado.
 */
describe('AgregadorService (metricas por empresa e idempotencia)', () => {
  function crear(estadoMarca = 1) {
    const repo = { guardarHechosVenta: vi.fn().mockResolvedValue(undefined) };
    const cliente = {
      query: vi.fn().mockImplementation((sql: string) => {
        if (String(sql).includes('eventos_procesados')) {
          return Promise.resolve({ rows: estadoMarca > 0 ? [{ event_id: 'e-1' }] : [], rowCount: estadoMarca });
        }
        return Promise.resolve({ rows: [], rowCount: 1 });
      }),
    };
    const pg = {
      query: vi.fn().mockResolvedValue([]),
      transaccion: vi.fn(async (fn: (c: unknown) => Promise<unknown>) => fn(cliente)),
    };
    const servicio = new AgregadorService(repo as never, pg as never);
    return { servicio, repo, pg, cliente };
  }

  it('debe actualizar el stock con la empresa del evento cuando llega stock.updated', async () => {
    const { servicio, pg } = crear();

    await servicio.procesarStockActualizado({
      empresa_id: 'e-1',
      sku: 'SKU-01',
      stock_restante: 7,
    });

    const [sql, params] = (pg.query as Mock).mock.calls[0];
    expect(sql).toContain('ON CONFLICT (empresa_id, sku)');
    expect(params).toEqual(['e-1', 'SKU-01', '', 7]);
  });

  it('debe agregar la venta geolocalizada al bucket de mercado cuando no hay empresa', async () => {
    const { servicio, repo, cliente } = crear();

    await servicio.procesarVentaGeolocalizada({
      order_id: 'o-1',
      vendedor_id: 'v-1',
      skus: ['SKU-01', 'SKU-02'],
      monto_cents: 1000,
      lat: 12.1,
      lng: -86.2,
    });

    expect(repo.guardarHechosVenta).toHaveBeenCalledTimes(1);
    expect(repo.guardarHechosVenta.mock.calls[0][2]).toBe(cliente);
    const llamadasMetricas = (cliente.query as Mock).mock.calls.filter((c: unknown[]) =>
      String(c[0]).includes('metricas_producto'),
    );
    expect(llamadasMetricas).toHaveLength(2);
    expect(llamadasMetricas.map((c: unknown[]) => c[1])).toEqual([
      [EMPRESA_MERCADO_ID, 'SKU-01', 500],
      [EMPRESA_MERCADO_ID, 'SKU-02', 500],
    ]);
  });

  it('debe ignorar una venta duplicada sin volver a agregar métricas', async () => {
    const { servicio, repo } = crear(0);

    await servicio.procesarVentaGeolocalizada(
      {
        order_id: 'o-1',
        vendedor_id: 'v-1',
        skus: ['SKU-01'],
        monto_cents: 1000,
        lat: 12.1,
        lng: -86.2,
      },
      '00000000-0000-0000-0000-000000000001',
    );

    expect(repo.guardarHechosVenta).not.toHaveBeenCalled();
  });
});
