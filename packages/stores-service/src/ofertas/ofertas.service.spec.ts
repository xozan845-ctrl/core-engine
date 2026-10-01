import { OfertasService } from './ofertas.service';
import { DomainError, ConflictError, NotFoundError } from '@core/shared';

/**
 * Ofertas: RN-01 (precio venta = base x (1 + margen)), RN-02 (no se
 * publica con stock < 1), propiedad de la oferta y sincronia con el
 * catalogo (REST, mocked — R-QA-2: nada de red en unit tests).
 */
describe('OfertasService (RN-01 / RN-02)', () => {
  const filaOferta = {
    id: 'of1',
    tienda_id: 't1',
    vendedor_id: 'v1',
    producto_id: 'p1',
    sku: 'SKU-01',
    producto_nombre: 'Teclado',
    margen: 25,
    precio_base_cents: 10000,
    precio_venta_cents: 12500,
    stock: 5,
    estado: 'activa',
    creado_en: '2026-09-01T00:00:00.000Z',
  };

  const productoExterno = {
    id: 'p1',
    sku: 'SKU-01',
    nombre: 'Teclado',
    precio_base: '100.00',
    stock: 5,
    estado: 'disponible',
  };

  const crear = () => {
    const pg = {
      query: jest.fn().mockResolvedValue([]),
      queryOne: jest.fn().mockResolvedValue(null),
    };
    const servicio = new OfertasService(pg as never);
    return { servicio, pg };
  };

  const fetchResponde = (ok: boolean, body: unknown) =>
    jest
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue({ ok, json: async () => body } as Response);

  afterEach(() => jest.restoreAllMocks());

  describe('publicar (H-01 / TC-02)', () => {
    it('debe rechazar el margen cuando esta fuera del rango 0-90 sin tocar la BD', async () => {
      const { servicio, pg } = crear();
      await expect(servicio.publicar('v1', 'p1', -1)).rejects.toThrow(/margen/i);
      await expect(servicio.publicar('v1', 'p1', 91)).rejects.toThrow(DomainError);
      expect(pg.queryOne).not.toHaveBeenCalled();
    });

    it('debe rechazar el alta cuando el vendedor no tiene tienda sin consultar el catalogo', async () => {
      const { servicio } = crear();
      const fetchSpy = jest.spyOn(globalThis, 'fetch');
      await expect(servicio.publicar('v1', 'p1', 25)).rejects.toThrow(/tienda/i);
      expect(fetchSpy).not.toHaveBeenCalled();
    });

    it('debe lanzar NotFoundError cuando el producto no existe en el catalogo', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValueOnce({ id: 't1' }); // tienda
      fetchResponde(false, { statusCode: 404 });
      await expect(servicio.publicar('v1', 'p1', 25)).rejects.toThrow(NotFoundError);
    });

    it('debe lanzar NotFoundError cuando el catalogo no responde (caida de red)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValueOnce({ id: 't1' });
      jest.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('red caida'));
      await expect(servicio.publicar('v1', 'p1', 25)).rejects.toThrow(NotFoundError);
    });

    it('debe prohibir publicar con stock 0 (RN-02)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValueOnce({ id: 't1' });
      fetchResponde(true, { ...productoExterno, stock: 0 });
      await expect(servicio.publicar('v1', 'p1', 25)).rejects.toThrow(ConflictError);
      const sqls = (pg.queryOne as jest.Mock).mock.calls.map((c) => c[0] as string);
      expect(sqls.some((s) => s.includes('INSERT INTO stores.ofertas'))).toBe(false);
    });

    it('debe devolver ConflictError cuando el producto ya esta publicado en la tienda', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock)
        .mockResolvedValueOnce({ id: 't1' }) // tienda
        .mockResolvedValueOnce({ id: 'of-duplicada' }); // oferta existente
      fetchResponde(true, productoExterno);
      await expect(servicio.publicar('v1', 'p1', 25)).rejects.toThrow(/publicado en tu tienda/);
    });

    it('debe calcular precio_venta = base x (1 + margen/100) y registrar historico (RN-01)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock)
        .mockResolvedValueOnce({ id: 't1' }) // tienda
        .mockResolvedValueOnce(null) // sin oferta duplicada
        .mockResolvedValueOnce(filaOferta); // INSERT ... RETURNING
      fetchResponde(true, productoExterno);

      const oferta = await servicio.publicar('v1', 'p1', 25);

      expect(oferta.precio_base).toBe('100.00');
      expect(oferta.precio_venta).toBe('125.00'); // 100 x 1.25
      expect(oferta.margen).toBe(25);
      expect(oferta.estado).toBe('activa');
      const [sqlInsert, params] = (pg.queryOne as jest.Mock).mock.calls[2];
      expect(sqlInsert).toContain('INSERT INTO stores.ofertas');
      expect(params[6]).toBe(10000); // precio_base_cents
      expect(params[7]).toBe(12500); // precio_venta_cents (enteros, A02)
      expect(pg.query).toHaveBeenCalledWith(
        expect.stringContaining('INSERT INTO stores.historico_precios'),
        ['of1', 12500],
      );
    });
  });

  describe('cambiarMargen', () => {
    it('debe rechazar el margen fuera de rango', async () => {
      const { servicio } = crear();
      await expect(servicio.cambiarMargen('of1', 'v1', 91)).rejects.toThrow(/margen/i);
    });

    it('debe lanzar NotFoundError cuando la oferta no existe', async () => {
      const { servicio } = crear();
      await expect(servicio.cambiarMargen('of404', 'v1', 10)).rejects.toThrow(NotFoundError);
    });

    it('debe prohibir editar la oferta de otro vendedor', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValueOnce({ ...filaOferta, vendedor_id: 'otro' });
      await expect(servicio.cambiarMargen('of1', 'v1', 10)).rejects.toThrow(/dueno/);
    });

    it('debe recalcular precio_venta desde la base y auditar el cambio cuando es el dueno', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock)
        .mockResolvedValueOnce({ ...filaOferta, vendedor_id: 'v1' }) // oferta
        .mockResolvedValueOnce({ ...filaOferta, margen: 10, precio_venta_cents: 11000 }); // UPDATE
      const oferta = await servicio.cambiarMargen('of1', 'v1', 10);
      expect(oferta.precio_venta).toBe('110.00'); // 100 x 1.10
      expect(pg.query).toHaveBeenCalledWith(
        expect.stringContaining('INSERT INTO stores.historico_precios'),
        ['of1', 12500, 11000], // precio anterior -> nuevo
      );
    });
  });

  describe('sincronizarStockDeOferta (RN-02)', () => {
    it('debe recortar el stock negativo a cero y marcar la oferta agotada', async () => {
      const { servicio, pg } = crear();
      await servicio.sincronizarStockDeOferta('SKU-01', -3);
      expect(pg.query).toHaveBeenCalledWith(expect.stringContaining("CASE WHEN $2 > 0"), [
        'SKU-01',
        0,
      ]);
    });
  });

  describe('porIds', () => {
    it('debe devolver vacio sin consultar cuando no hay ids', async () => {
      const { servicio, pg } = crear();
      expect(await servicio.porIds([])).toEqual([]);
      expect(pg.query).not.toHaveBeenCalled();
    });
  });
});
