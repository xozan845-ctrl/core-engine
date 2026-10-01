import { TiendasService } from './tiendas.service';
import { DomainError, ConflictError } from '@core/shared';

/** Tiendas: una por vendedor (Tabla 11) con nombre de 2 a 100 caracteres. */
describe('TiendasService (una tienda por vendedor)', () => {
  const crear = () => {
    const pg = {
      queryOne: jest.fn().mockResolvedValue(null),
      query: jest.fn().mockResolvedValue([]),
    };
    const servicio = new TiendasService(pg as never);
    return { servicio, pg };
  };

  const tienda = {
    id: 't1',
    vendedor_id: 'v1',
    nombre: 'Mi tienda',
    descripcion: 'venta de perifericos',
    creado_en: '2026-09-01T00:00:00.000Z',
  };

  describe('crear', () => {
    it('debe rechazar el nombre cuando tiene menos de 2 caracteres', async () => {
      const { servicio, pg } = crear();
      await expect(servicio.crear('v1', 'X')).rejects.toThrow(/entre 2 y 100/);
      expect(pg.queryOne).not.toHaveBeenCalled();
    });

    it('debe rechazar el nombre cuando supera los 100 caracteres', async () => {
      const { servicio } = crear();
      await expect(servicio.crear('v1', 'x'.repeat(101))).rejects.toThrow(DomainError);
    });

    it('debe devolver ConflictError cuando el vendedor ya tiene tienda', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValueOnce(tienda); // deVendedor -> existe
      await expect(servicio.crear('v1', 'Otra tienda')).rejects.toThrow(ConflictError);
      const sqls = (pg.queryOne as jest.Mock).mock.calls.map((c) => c[0] as string);
      expect(sqls.some((s) => s.includes('INSERT INTO stores.tiendas'))).toBe(false);
    });

    it('debe recortar el nombre y crear la tienda cuando todo es valido', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock)
        .mockResolvedValueOnce(null) // sin tienda previa
        .mockResolvedValueOnce(tienda); // INSERT ... RETURNING
      const creada = await servicio.crear('v1', '  Mi tienda  ', '  venta  ');
      expect(creada.id).toBe('t1');
      const [sql, params] = (pg.queryOne as jest.Mock).mock.calls[1];
      expect(sql).toContain('INSERT INTO stores.tiendas');
      expect(params[1]).toBe('v1');
      expect(params[2]).toBe('Mi tienda'); // trim aplicado
      expect(params[3]).toBe('venta');
    });

    it('debe lanzar DomainError cuando el INSERT no devuelve fila', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock)
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null);
      await expect(servicio.crear('v1', 'Mi tienda')).rejects.toThrow(/no se pudo crear/i);
    });
  });

  describe('deVendedor', () => {
    it('debe consultar la tienda por vendedor_id', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(tienda);
      expect(await servicio.deVendedor('v1')).toEqual(tienda);
      expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('stores.tiendas'), ['v1']);
    });
  });
});
