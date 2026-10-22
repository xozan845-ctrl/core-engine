import type { Mock } from 'vitest';
import { TiendasService } from './tiendas.service';
import { DomainError } from '@core/shared';

describe('TiendasService (multiples tiendas por empresa)', () => {
  const crear = () => {
    const pg = {
      queryOne: vi.fn().mockResolvedValue(null),
      query: vi.fn().mockResolvedValue([]),
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
      await expect(servicio.crear('e1', 'v1', 25, 'X')).rejects.toThrow(/entre 2 y 100/);
      expect(pg.queryOne).not.toHaveBeenCalled();
    });

    it('debe rechazar el nombre cuando supera los 100 caracteres', async () => {
      const { servicio } = crear();
      await expect(servicio.crear('e1', 'v1', 25, 'x'.repeat(101))).rejects.toThrow(DomainError);
    });

    it('debe recortar el nombre y crear la tienda cuando todo es valido', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(tienda);
      const creada = await servicio.crear('e1', 'v1', 25, '  Mi tienda  ', '  venta  ');
      expect(creada.id).toBe('t1');
      const [sql, params] = (pg.queryOne as Mock).mock.calls[0];
      expect(sql).toContain('INSERT INTO stores.tiendas');
      expect(params[1]).toBe('e1');
      expect(params[2]).toBe('v1');
      expect(params[3]).toBe(25);
      expect(params[4]).toBe('Mi tienda');
      expect(params[5]).toBe('venta');
    });

    it('debe lanzar DomainError cuando el INSERT no devuelve fila', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(null);
      await expect(servicio.crear('e1', 'v1', 25, 'Mi tienda')).rejects.toThrow(/no se pudo crear/i);
    });
  });

  describe('deVendedor', () => {
    it('debe consultar la tienda por vendedor_id', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(tienda);
      expect(await servicio.deVendedor('v1')).toEqual(tienda);
      expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('stores.tiendas'), ['v1']);
    });
  });

  describe('encontrarPorId', () => {
    it('debe consultar la tienda por id', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(tienda);
      expect(await servicio.encontrarPorId('t1')).toEqual(tienda);
      expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('WHERE id = $1'), ['t1']);
    });
  });
});
