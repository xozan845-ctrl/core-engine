import { ComisionesService } from './comisiones.service';

describe('ComisionesService (liquidación por línea)', () => {
  it('cobra plataforma primero y reparte el neto según participación', () => {
    const servicio = new ComisionesService({} as never);
    expect(servicio.calcular(10_000, 60, 0.12)).toEqual({
      comision_cents: 1_200,
      monto_vendedor_cents: 5_280,
      monto_empresario_cents: 3_520,
    });
  });

  it('asigna todo el neto al vendedor con participación 100 % (ADR-25)', () => {
    const servicio = new ComisionesService({} as never);
    expect(servicio.calcular(10_000, 100, 0.12)).toEqual({
      comision_cents: 1_200,
      monto_vendedor_cents: 8_800,
      monto_empresario_cents: 0,
    });
  });

  it('asigna todo el neto al empresario con participación 0 %', () => {
    const servicio = new ComisionesService({} as never);
    expect(servicio.calcular(10_000, 0, 0.12)).toEqual({
      comision_cents: 1_200,
      monto_vendedor_cents: 0,
      monto_empresario_cents: 8_800,
    });
  });

  it('rechaza una participación fuera de rango', () => {
    const servicio = new ComisionesService({} as never);
    expect(() => servicio.calcular(10_000, 101, 0.12)).toThrow('participacion');
  });

  it('es idempotente por orden y línea', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue({ id: 'comision-1' }) };
    const servicio = new ComisionesService(pg as never);
    await expect(servicio.acreditarLinea({
      orderId: 'orden-1', lineaClave: 'oferta-1', empresaId: 'empresa-1',
      tiendaId: 'tienda-1', vendedorId: 'vendedor-1', ventaCents: 10_000,
      participacion: 60,
    })).resolves.toBeNull();
    expect(pg.queryOne).toHaveBeenCalledTimes(1);
    expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('linea_clave = $2'), ['orden-1', 'oferta-1']);
  });
});
