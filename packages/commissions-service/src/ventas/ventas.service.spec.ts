import type { Mock } from 'vitest';
import 'reflect-metadata';
import { VentasService } from './ventas.service';
import type { PgService } from '@core/shared';

function fabricar(): { pg: Record<string, Mock>; servicio: VentasService } {
  const pg = { query: vi.fn(), queryOne: vi.fn() };
  const servicio = new VentasService(pg as unknown as PgService);
  return { pg, servicio };
}

describe('VentasService.estadoCuentaEmpresa (ADR-25, R-DI-5)', () => {
  it('debe consolidar las comisiones y liquidaciones de la empresa', async () => {
    const { pg, servicio } = fabricar();
    pg.query
      .mockResolvedValueOnce([
        {
          order_id: 'o-1',
          venta_cents: 115000,
          comision_cents: 13800,
          monto_empresario_cents: 12600,
          estado: 'acreditada',
          creado_en: '2026-09-10T00:00:00.000Z',
        },
        {
          order_id: 'o-2',
          venta_cents: 50000,
          comision_cents: 6000,
          monto_empresario_cents: 5400,
          estado: 'liquidada',
          creado_en: '2026-09-11T00:00:00.000Z',
        },
      ])
      .mockResolvedValueOnce([
        {
          id: 'liq-1',
          empresa_id: 'empresa-1',
          periodo_inicio: '2026-09-01',
          periodo_fin: '2026-09-15',
          monto_cents: 18000,
          estado: 'aprobada',
          creado_en: '2026-09-16T00:00:00.000Z',
        },
      ]);

    const resultado = await servicio.estadoCuentaEmpresa('empresa-1');

    expect(resultado.empresa_id).toBe('empresa-1');
    expect(resultado.comisiones_pendientes).toBe('126.00');
    expect(resultado.ventas).toEqual([
      {
        order_id: 'o-1',
        venta: '1150.00',
        comision: '138.00',
        monto_empresario: '126.00',
        estado: 'acreditada',
        fecha: '2026-09-10T00:00:00.000Z',
      },
      {
        order_id: 'o-2',
        venta: '500.00',
        comision: '60.00',
        monto_empresario: '54.00',
        estado: 'liquidada',
        fecha: '2026-09-11T00:00:00.000Z',
      },
    ]);
    expect(resultado.liquidaciones).toEqual([
      {
        id: 'liq-1',
        empresa_id: 'empresa-1',
        periodo_inicio: '2026-09-01',
        periodo_fin: '2026-09-15',
        monto_cents: 18000,
        estado: 'aprobada',
        creado_en: '2026-09-16T00:00:00.000Z',
        monto: '180.00',
      },
    ]);
    expect(pg.query.mock.calls[0][1]).toEqual(['empresa-1']);
    expect(pg.query.mock.calls[1][1]).toEqual(['empresa-1']);
    expect(pg.query.mock.calls[0][0]).toContain('WHERE empresa_id = $1');
    expect(pg.query.mock.calls[1][0]).toContain('liquidaciones_empresas');
  });

  it('debe devolver un estado vacio cuando la empresa no tiene movimientos', async () => {
    const { pg, servicio } = fabricar();
    pg.query.mockResolvedValueOnce([]).mockResolvedValueOnce([]);

    const resultado = await servicio.estadoCuentaEmpresa('empresa-1');

    expect(resultado).toEqual({
      empresa_id: 'empresa-1',
      ventas: [],
      comisiones_pendientes: '0.00',
      liquidaciones: [],
    });
  });
});
