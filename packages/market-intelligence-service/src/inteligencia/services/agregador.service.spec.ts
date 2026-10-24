import type { Mock } from 'vitest';
import { AgregadorService } from './agregador.service';
import { EMPRESA_MERCADO_ID } from '@core/shared';

/**
 * ADR-27: las métricas de producto se agregan por empresa. Las ventas
 * geolocalizadas que no identifican empresa usan el bucket de mercado.
 */
describe('AgregadorService (metricas por empresa)', () => {
  function crear() {
    const repo = { guardarHechosVenta: vi.fn().mockResolvedValue(undefined) };
    const pg = { query: vi.fn().mockResolvedValue([]) };
    const servicio = new AgregadorService(repo as never, pg as never);
    return { servicio, repo, pg };
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
    const { servicio, repo, pg } = crear();

    await servicio.procesarVentaGeolocalizada({
      order_id: 'o-1',
      vendedor_id: 'v-1',
      skus: ['SKU-01', 'SKU-02'],
      monto_cents: 1000,
      lat: 12.1,
      lng: -86.2,
    });

    expect(repo.guardarHechosVenta).toHaveBeenCalledTimes(1);
    const llamadasMetricas = (pg.query as Mock).mock.calls.filter((c) =>
      String(c[0]).includes('metricas_producto'),
    );
    expect(llamadasMetricas).toHaveLength(2);
    expect(llamadasMetricas.map((c) => c[1])).toEqual([
      [EMPRESA_MERCADO_ID, 'SKU-01', 500],
      [EMPRESA_MERCADO_ID, 'SKU-02', 500],
    ]);
  });
});
