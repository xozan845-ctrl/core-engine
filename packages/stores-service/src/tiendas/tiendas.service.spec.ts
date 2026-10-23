import type { Mock } from 'vitest';
import { ConflictError, DomainError, ForbiddenError, NotFoundError } from '@core/shared';
import { TiendasService } from './tiendas.service';

const tienda = {
  id: 't1',
  empresa_id: 'e1',
  propietario_usuario_id: 'v1',
  participacion_porcentaje: 25,
  vendedor_id: 'v1',
  nombre: 'Mi tienda',
  slug: 'mi-tienda',
  descripcion: 'venta de perifericos',
  creado_en: '2026-09-01T00:00:00.000Z',
};

function crearServicio(): { servicio: TiendasService; pg: { queryOne: Mock; query: Mock } } {
  const pg = {
    queryOne: vi.fn().mockResolvedValue(null),
    query: vi.fn().mockResolvedValue([]),
  };
  return { servicio: new TiendasService(pg as never), pg };
}

describe('TiendasService (multiples tiendas por empresa)', () => {
  describe('crear (ADR-26)', () => {
    it('debe rechazar el nombre cuando tiene menos de 2 caracteres', async () => {
      const { servicio, pg } = crearServicio();
      await expect(servicio.crear('e1', 'v1', 25, 'X')).rejects.toThrow(/entre 2 y 100/);
      expect(pg.queryOne).not.toHaveBeenCalled();
    });

    it('debe rechazar el nombre cuando supera los 100 caracteres', async () => {
      const { servicio } = crearServicio();
      await expect(servicio.crear('e1', 'v1', 25, 'x'.repeat(101))).rejects.toThrow(DomainError);
    });

    it('debe derivar el slug, recortar los textos y crear la tienda', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(tienda);

      const creada = await servicio.crear('e1', 'v1', 25, '  Mi tienda  ', '  venta  ');

      expect(creada.id).toBe('t1');
      expect(pg.queryOne).toHaveBeenNthCalledWith(
        1,
        expect.stringContaining('FROM stores.tienda_slugs'),
        ['mi-tienda'],
      );
      const [sql, params] = pg.queryOne.mock.calls[1];
      expect(sql).toContain('INSERT INTO stores.tiendas');
      expect(params[1]).toBe('e1');
      expect(params[2]).toBe('v1');
      expect(params[3]).toBe(25);
      expect(params[4]).toBe('Mi tienda');
      expect(params[5]).toBe('venta');
      expect(params[6]).toBe('mi-tienda');
    });

    it('debe resolver una colisión de slug con sufijo -2', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce({ ok: 1 })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(tienda);

      await servicio.crear('e1', 'v1', 25, 'Mi tienda');

      expect(pg.queryOne).toHaveBeenNthCalledWith(
        2,
        expect.stringContaining('FROM stores.tienda_slugs'),
        ['mi-tienda-2'],
      );
    });

    it('debe aceptar un slug explícito normalizado', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce(null).mockResolvedValueOnce(tienda);

      await servicio.crear('e1', 'v1', 25, 'Mi tienda', undefined, '  Tienda Piloto  ');

      expect(pg.queryOne).toHaveBeenNthCalledWith(1, expect.any(String), ['tienda-piloto']);
      expect(pg.queryOne.mock.calls[1][1][6]).toBe('tienda-piloto');
    });

    it('debe rechazar un slug explícito ya usado', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce({ ok: 1 });

      await expect(
        servicio.crear('e1', 'v1', 25, 'Mi tienda', undefined, 'ocupado'),
      ).rejects.toBeInstanceOf(ConflictError);
      expect(pg.queryOne).toHaveBeenCalledTimes(1);
    });

    it('debe reintentar cuando otra petición gana la carrera del slug', async () => {
      const { servicio, pg } = crearServicio();
      let llamada = 0;
      pg.queryOne.mockImplementation(async () => {
        llamada += 1;
        if (llamada === 1) return null; // slug libre
        if (llamada === 2) throw { code: '23505' }; // INSERT pierde la carrera
        if (llamada === 3) return null; // segundo intento
        return tienda; // INSERT correcto
      });

      await expect(servicio.crear('e1', 'v1', 25, 'Mi tienda')).resolves.toEqual(tienda);
      expect(pg.queryOne).toHaveBeenCalledTimes(4);
    });

    it('debe lanzar DomainError cuando el INSERT no devuelve fila', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce(null).mockResolvedValueOnce(null);

      await expect(servicio.crear('e1', 'v1', 25, 'Mi tienda')).rejects.toThrow(/no se pudo crear/i);
    });
  });

  describe('consultas', () => {
    it('debe consultar la tienda por vendedor_id incluyendo el slug', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValue(tienda);
      expect(await servicio.deVendedor('v1')).toEqual(tienda);
      expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('slug'), ['v1']);
    });

    it('debe listar las tiendas de la empresa incluyendo el slug', async () => {
      const { servicio, pg } = crearServicio();
      pg.query.mockResolvedValue([tienda]);
      expect(await servicio.deEmpresa('e1')).toEqual([tienda]);
      expect(pg.query).toHaveBeenCalledWith(
        expect.stringContaining('WHERE empresa_id = $1'),
        ['e1'],
      );
    });

    it('debe resolver la tienda por id', async () => {
      const { servicio, pg } = crearServicio();
      const id = '11111111-2222-3333-4444-555555555555';
      pg.queryOne.mockResolvedValue({ ...tienda, id, es_actual: true });
      expect(await servicio.encontrarPorId(id)).toEqual({ ...tienda, id });
      expect(pg.queryOne).toHaveBeenCalledWith(
        expect.stringContaining('WHERE t.id = $1'),
        [id],
      );
    });
  });

  describe('resolver (UUID o slug, ADR-26)', () => {
    it('debe resolver por UUID sin redirigir', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValue({ ...tienda, es_actual: true });

      await expect(servicio.resolver(tienda.id)).resolves.toEqual({ tienda });
    });

    it('debe resolver el slug actual sin redirigir', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValue({ ...tienda, es_actual: true });

      await expect(servicio.resolver('mi-tienda')).resolves.toEqual({ tienda });
      expect(pg.queryOne).toHaveBeenCalledWith(
        expect.stringContaining('FROM stores.tienda_slugs'),
        ['mi-tienda'],
      );
    });

    it('debe redirigir al slug actual cuando el consultado es anterior', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValue({ ...tienda, slug: 'mi-tienda-nueva', es_actual: false });

      await expect(servicio.resolver('mi-tienda-vieja')).resolves.toEqual({
        tienda: { ...tienda, slug: 'mi-tienda-nueva' },
        redirigirA: '/api/v1/tiendas/mi-tienda-nueva',
      });
    });

    it('debe devolver null cuando el UUID o slug no existe', async () => {
      const { servicio } = crearServicio();
      await expect(servicio.resolver('no-existe')).resolves.toBeNull();
    });
  });

  describe('actualizarSlug (ADR-26)', () => {
    it('debe cambiar el slug cuando el actor es el propietario', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce({ ...tienda, es_actual: true })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce({ ...tienda, slug: 'mi-tienda-nueva' });

      await expect(
        servicio.actualizarSlug('v1', 't1', '  Mi Tienda Nueva  '),
      ).resolves.toEqual({ ...tienda, slug: 'mi-tienda-nueva' });
      expect(pg.queryOne).toHaveBeenNthCalledWith(
        3,
        expect.stringContaining('UPDATE stores.tiendas SET slug=$1'),
        ['mi-tienda-nueva', 't1'],
      );
    });

    it('debe ser idempotente cuando el slug no cambia', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce({ ...tienda, es_actual: true });

      await expect(servicio.actualizarSlug('v1', 't1', 'mi-tienda')).resolves.toEqual(tienda);
      expect(pg.queryOne).toHaveBeenCalledTimes(1);
    });

    it('debe rechazar a quien no es propietario', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne.mockResolvedValueOnce({ ...tienda, es_actual: true });

      await expect(servicio.actualizarSlug('u-x', 't1', 'nueva-direccion')).rejects.toBeInstanceOf(
        ForbiddenError,
      );
      expect(pg.queryOne).toHaveBeenCalledTimes(1);
    });

    it('debe rechazar un slug ya usado (incluido histórico)', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce({ ...tienda, es_actual: true })
        .mockResolvedValueOnce({ ok: 1 });

      await expect(servicio.actualizarSlug('v1', 't1', 'ocupado')).rejects.toBeInstanceOf(
        ConflictError,
      );
    });

    it('debe traducir una carrera de unicidad a ConflictError', async () => {
      const { servicio, pg } = crearServicio();
      pg.queryOne
        .mockResolvedValueOnce({ ...tienda, es_actual: true })
        .mockResolvedValueOnce(null)
        .mockRejectedValueOnce({ code: '23505' });

      await expect(servicio.actualizarSlug('v1', 't1', 'nueva-direccion')).rejects.toBeInstanceOf(
        ConflictError,
      );
    });

    it('debe devolver NotFound si la tienda no existe', async () => {
      const { servicio } = crearServicio();
      await expect(servicio.actualizarSlug('v1', 'no-existe', 'nueva-direccion')).rejects.toBeInstanceOf(
        NotFoundError,
      );
    });
  });
});
