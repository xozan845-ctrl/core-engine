import { describe, expect, it, vi } from 'vitest';
import { MetricasService } from './metricas.service';

function crearServicio() {
  const client = {
    query: vi.fn().mockResolvedValue({ rows: [] }),
  };
  const pg = {};
  return { servicio: new MetricasService(pg as never), client };
}

describe('MetricasService', () => {
  it('debe sumar una métrica mensual mediante un upsert atómico', async () => {
    const { servicio, client } = crearServicio();

    await servicio.sumarMetrica(client as never, 'gmv', 12500, 3);

    expect(client.query).toHaveBeenCalledOnce();
    const [sql, params] = client.query.mock.calls[0];
    expect(sql).toContain('INSERT INTO finance.metricas_mensuales');
    expect(sql).toContain('ON CONFLICT (periodo, tipo) DO UPDATE');
    expect(sql).toContain('monto_cents + EXCLUDED.monto_cents');
    expect(params).toEqual(['gmv', 12500, 3]);
  });

  it('debe incrementar las métricas del vendedor y conservar sus deltas', async () => {
    const { servicio, client } = crearServicio();

    await servicio.incrementarVendedor(client as never, 'vendedor-1', {
      ventas: 2,
      monto_cents: 24000,
      comision_cents: 3600,
      devoluciones: 1,
    });

    expect(client.query).toHaveBeenCalledOnce();
    const [sql, params] = client.query.mock.calls[0];
    expect(sql).toContain('INSERT INTO finance.metricas_vendedores');
    expect(sql).toContain('ON CONFLICT (vendedor_id, periodo) DO UPDATE');
    expect(sql).toContain('ventas          = finance.metricas_vendedores.ventas + EXCLUDED.ventas');
    expect(params).toEqual(['vendedor-1', 2, 24000, 3600, 1]);
  });

  it('debe usar cero en los deltas omitidos al registrar un vendedor', async () => {
    const { servicio, client } = crearServicio();

    await servicio.incrementarVendedor(client as never, 'vendedor-2', {});

    expect(client.query.mock.calls[0][1]).toEqual(['vendedor-2', 0, 0, 0, 0]);
  });
});
