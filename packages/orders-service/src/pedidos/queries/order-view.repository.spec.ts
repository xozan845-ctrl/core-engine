import type { Mock } from 'vitest';
import 'reflect-metadata';
import { OrderViewRepository } from './order-view.repository';
import type { PgService } from '@core/shared';

function fabricar(): { pg: Record<string, Mock>; repositorio: OrderViewRepository } {
  const pg = { query: vi.fn().mockResolvedValue([]) };
  const repositorio = new OrderViewRepository(pg as unknown as PgService);
  return { pg, repositorio };
}

describe('OrderViewRepository.listarDeTienda (R-DS-3, ADR-25)', () => {
  it('debe filtrar la misma linea por tienda, empresa, vendedor y estado', async () => {
    const { pg, repositorio } = fabricar();

    await repositorio.listarDeTienda('t-1', 'pagada', 'u-v', 'e-1');

    const [sql, params] = pg.query.mock.calls[0] as [string, unknown[]];
    expect(sql).toContain("item->>'tienda_id' = $1");
    expect(sql).toContain("item->>'empresa_id' = $2");
    expect(sql).toContain("item->>'vendedor_id' = $3");
    expect(sql).toContain('estado = $4');
    expect(params).toEqual(['t-1', 'e-1', 'u-v', 'pagada']);
  });

  it('debe mantener el filtro solo por tienda cuando no hay empresa ni vendedor', async () => {
    const { pg, repositorio } = fabricar();

    await repositorio.listarDeTienda('t-1');

    const [sql, params] = pg.query.mock.calls[0] as [string, unknown[]];
    expect(sql).toContain("item->>'tienda_id' = $1");
    expect(sql).not.toContain("item->>'empresa_id'");
    expect(sql).not.toContain("item->>'vendedor_id'");
    expect(params).toEqual(['t-1']);
  });
});

describe('OrderViewRepository.listarDeEmpresa (R-DS-3, ADR-25)', () => {
  it('debe filtrar por empresa, tienda, estado y limite', async () => {
    const { pg, repositorio } = fabricar();

    await repositorio.listarDeEmpresa('e-1', {
      estado: 'pagada',
      tiendaId: 't-1',
      limite: 25,
    });

    const [sql, params] = pg.query.mock.calls[0] as [string, unknown[]];
    expect(sql).toContain("item->>'empresa_id' = $1");
    expect(sql).toContain("item->>'tienda_id' = $2");
    expect(sql).toContain('estado = $3');
    expect(sql).toContain('LIMIT $4');
    expect(params).toEqual(['e-1', 't-1', 'pagada', 25]);
  });

  it('debe usar el limite por defecto cuando no se indica', async () => {
    const { pg, repositorio } = fabricar();

    await repositorio.listarDeEmpresa('e-1');

    const [sql, params] = pg.query.mock.calls[0] as [string, unknown[]];
    expect(sql).toContain("item->>'empresa_id' = $1");
    expect(sql).toContain('LIMIT $2');
    expect(params).toEqual(['e-1', 100]);
  });
});
