import type { Mock } from 'vitest';
import { BucketsService } from './buckets.service';
import { DomainError, NotFoundError } from '@core/shared';
import type { StorageConfig } from '../config/storage.config';

const filaBucket = (extra: Record<string, unknown> = {}) => ({
  tenant_id: '11111111-1111-4111-8111-111111111111',
  bucket: 't-11111111-1111-4111-8111-111111111111',
  cuota_bytes: '1000',
  usado_bytes: '250',
  estado: 'activo',
  creado_en: '2026-10-16T00:00:00.000Z',
  actualizado_en: '2026-10-16T00:00:00.000Z',
  objetos: 2,
  ...extra,
});

const crear = () => {
  const pg = {
    query: vi.fn().mockResolvedValue([]),
    queryOne: vi.fn().mockResolvedValue(null),
    transaccion: vi.fn(),
  };
  const almacen = {
    asegurarBucket: vi.fn().mockResolvedValue(undefined),
    existeBucket: vi.fn(),
    urlSubida: vi.fn(),
    urlDescarga: vi.fn(),
    estadisticas: vi.fn(),
    eliminar: vi.fn(),
  };
  const config = { prefijoBucket: 't-', cuotaDefaultBytes: 5000 } as unknown as StorageConfig;
  return {
    servicio: new BucketsService(pg as never, almacen as never, config),
    pg,
    almacen,
  };
};

describe('BucketsService (multitenancy por bucket y cuotas, ADR-24)', () => {
  it('debe derivar el nombre del bucket del tenant en minusculas', () => {
    const { servicio } = crear();
    expect(servicio.nombreBucket('ABC-DEF')).toBe('t-abc-def');
  });

  it('debe asegurar bucket fisico y fila de control de forma idempotente', async () => {
    const { servicio, pg, almacen } = crear();
    (pg.queryOne as Mock).mockResolvedValue(filaBucket());

    const estado = await servicio.asegurar('11111111-1111-4111-8111-111111111111');

    expect(almacen.asegurarBucket).toHaveBeenCalledWith('t-11111111-1111-4111-8111-111111111111');
    const [sql, params] = (pg.query as Mock).mock.calls[0];
    expect(sql).toContain('INSERT INTO storage.buckets');
    expect(sql).toContain('ON CONFLICT (tenant_id) DO NOTHING');
    expect(params).toEqual(['11111111-1111-4111-8111-111111111111', 't-11111111-1111-4111-8111-111111111111', 5000]);
    expect(estado.cuota_bytes).toBe(1000);
    expect(estado.usado_bytes).toBe(250);
    expect(estado.disponible_bytes).toBe(750);
    expect(estado.objetos).toBe(2);
  });

  it('debe lanzar NotFoundError cuando el tenant no tiene bucket', async () => {
    const { servicio } = crear();
    await expect(servicio.estado('sin-bucket')).rejects.toThrow(NotFoundError);
  });

  it('debe listar los buckets paginados (admin multi-tenant)', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue({ n: 1 });
    (pg.query as Mock).mockResolvedValue([filaBucket()]);

    const pagina = await servicio.listar(1, 10);

    expect(pagina.total).toBe(1);
    expect(pagina.items[0].tenant_id).toBe('11111111-1111-4111-8111-111111111111');
    const [sql, params] = (pg.query as Mock).mock.calls[0];
    expect(sql).toContain('ORDER BY b.creado_en DESC');
    expect(params).toEqual([10, 0]);
  });

  it('debe rechazar cuotas invalidas', async () => {
    const { servicio } = crear();
    await expect(servicio.actualizarCuota('t1', 0)).rejects.toThrow(DomainError);
    await expect(servicio.actualizarCuota('t1', 1.5)).rejects.toThrow(DomainError);
  });

  it('no debe permitir una cuota por debajo del uso actual', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue(filaBucket());
    await expect(servicio.actualizarCuota('t1', 100)).rejects.toThrow(/CUOTA_MENOR_AL_USO|no puede ser menor/);
  });

  it('debe actualizar la cuota cuando es valida', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue(filaBucket({ cuota_bytes: '9000' }));

    const estado = await servicio.actualizarCuota('t1', 9000);

    const [sql, params] = (pg.query as Mock).mock.calls[0];
    expect(sql).toContain('SET cuota_bytes = $2');
    expect(params).toEqual(['t1', 9000]);
    expect(estado.cuota_bytes).toBe(9000);
  });

  it('debe cambiar el estado del almacenamiento del tenant', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue(filaBucket({ estado: 'suspendido' }));

    const estado = await servicio.cambiarEstado('t1', 'suspendido');

    expect((pg.query as Mock).mock.calls[0][0]).toContain('SET estado = $2');
    expect(estado.estado).toBe('suspendido');
  });

  it('debe validar la cuota al pedir una subida', () => {
    const { servicio } = crear();
    const base = {
      tenant_id: 't1',
      bucket: 't-t1',
      cuota_bytes: 1000,
      usado_bytes: 900,
      disponible_bytes: 100,
      objetos: 0,
      estado: 'activo' as const,
      creado_en: '',
      actualizado_en: '',
    };
    expect(() => servicio.verificarCuota(base, 100)).not.toThrow();
    expect(() => servicio.verificarCuota(base, 101)).toThrow(/CUOTA_EXCEDIDA|cuota/);
    expect(() => servicio.verificarCuota({ ...base, estado: 'suspendido' }, 1)).toThrow(
      /suspendido/,
    );
  });

  it('debe sumar y restar uso dentro de la transaccion del objeto', async () => {
    const { servicio } = crear();
    const client = { query: vi.fn().mockResolvedValue({ rowCount: 1 }) };

    await servicio.sumarUsoEnTransaccion(client as never, 't1', 300);
    await servicio.restarUsoEnTransaccion(client as never, 't1', 300);

    expect(client.query.mock.calls[0][0]).toContain('usado_bytes = usado_bytes + $2');
    expect(client.query.mock.calls[1][0]).toContain('GREATEST(0, usado_bytes - $2)');
  });

  it('debe recalcular el uso desde la metadata activa', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue(filaBucket({ usado_bytes: '333' }));

    const estado = await servicio.recalcularUso('t1');

    const [sql] = (pg.query as Mock).mock.calls[0];
    expect(sql).toContain("o.estado = 'activo'");
    expect(estado.usado_bytes).toBe(333);
  });
});
