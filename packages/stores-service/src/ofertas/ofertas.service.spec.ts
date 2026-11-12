import type { Mock } from 'vitest';
import { OfertasService } from './ofertas.service';
import { NotFoundError } from '@core/shared';

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
    empresa_id: 'e1',
    participacion_porcentaje: 25,
    item_type: 'producto',
    item_id: 'item-p1',
    sku: 'SKU-01',
    producto_nombre: 'Teclado',
    atributos: { color: 'rojo' },
    imagen_objeto_id: 'img-1',
    margen: 25,
    precio_base_cents: 10000,
    precio_venta_cents: 12500,
    stock: 5,
    estado: 'activa',
    creado_en: '2026-09-01T00:00:00.000Z',
  };


  const crear = () => {
    const pg = {
      query: vi.fn().mockResolvedValue([]),
      queryOne: vi.fn().mockResolvedValue(null),
    };
    const servicio = new OfertasService(pg as never);
    return { servicio, pg };
  };

  const fetchResponde = (ok: boolean, body: unknown) =>
    vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue({ ok, json: async () => body } as Response);

  afterEach(() => vi.restoreAllMocks());

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
      (pg.queryOne as Mock).mockResolvedValueOnce({ ...filaOferta, vendedor_id: 'otro' });
      await expect(servicio.cambiarMargen('of1', 'v1', 10)).rejects.toBeInstanceOf(NotFoundError);
    });

    it('debe recalcular precio_venta desde la base y auditar el cambio cuando es el dueno', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...filaOferta, vendedor_id: 'v1' }) // oferta
        .mockResolvedValueOnce({ ...filaOferta, margen: 10, precio_venta_cents: 11000 }); // UPDATE
      const oferta = await servicio.cambiarMargen('of1', 'v1', 10);
      expect(oferta.precio_venta).toBe('110.00'); // 100 x 1.10
      expect(pg.query).toHaveBeenCalledWith(
        expect.stringContaining('INSERT INTO stores.historico_precios'),
        ['of1', 12500, 11000], // precio anterior -> nuevo
      );
    });

    it('debe lanzar NotFoundError cuando el UPDATE no devuelve fila', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...filaOferta, vendedor_id: 'v1' })
        .mockResolvedValueOnce(null);
      await expect(servicio.cambiarMargen('of1', 'v1', 10)).rejects.toBeInstanceOf(NotFoundError);
    });
  });

  describe('publicarItem (fase 2)', () => {
    it('publica un servicio sin exigir stock y conserva item_type/item_id', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ id: 't1', empresa_id: 'e1' })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce({ id: 'of-servicio' })
        .mockResolvedValueOnce({
          ...filaOferta,
          item_type: 'servicio',
          item_id: 'item-servicio',
          stock: null,
        });
      fetchResponde(true, {
        item_id: 'item-servicio',
        empresa_id: 'e1',
        tipo: 'servicio',
        codigo: 'SERV-001',
        nombre: 'Consulta',
        precio: '100.00',
        stock: null,
        track_inventory: false,
        duration_minutes: 60,
        atributos: { especialidad: 'general' },
        estado: 'activo',
        source_type: 'servicio',
        source_id: 'service-1',
      });

      const oferta = await servicio.publicarItem('v1', 't1', 'item-servicio', 20);

      expect(oferta).toMatchObject({ item_type: 'servicio', item_id: 'item-servicio', stock: null });
      expect(globalThis.fetch).toHaveBeenCalledWith(
        expect.stringContaining('/internal/items/item-servicio'),
        expect.any(Object),
      );
      const insert = (pg.queryOne as Mock).mock.calls[2];
      expect(insert[0]).toContain('item_type');
      expect(insert[1]).toContain('servicio');
    });

    it('rechaza un item inexistente o de otra empresa', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce({ id: 't1', empresa_id: 'e1' });
      fetchResponde(false, {});
      await expect(servicio.publicarItem('v1', 't1', 'item-x', 10)).rejects.toBeInstanceOf(NotFoundError);

      (pg.queryOne as Mock).mockReset().mockResolvedValueOnce({ id: 't1', empresa_id: 'e1' });
      fetchResponde(true, {
        item_id: 'item-1', empresa_id: 'e-otra', tipo: 'producto', codigo: 'P1',
        nombre: 'P1', precio: '10.00', stock: 5, track_inventory: true,
        duration_minutes: null, atributos: {}, estado: 'activo', source_type: 'producto', source_id: 'p1',
      });
      await expect(servicio.publicarItem('v1', 't1', 'item-1', 10)).rejects.toThrow(/solo puede publicar items/);
    });

    it('rechaza items inactivos o sin stock', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce({ id: 't1', empresa_id: 'e1' });
      fetchResponde(true, {
        item_id: 'item-1', empresa_id: 'e1', tipo: 'producto', codigo: 'P1',
        nombre: 'P1', precio: '10.00', stock: 5, track_inventory: true,
        duration_minutes: null, atributos: {}, estado: 'inactivo', source_type: 'producto', source_id: 'p1',
      });
      await expect(servicio.publicarItem('v1', 't1', 'item-1', 10)).rejects.toThrow(/inactivo/);

      (pg.queryOne as Mock).mockReset().mockResolvedValueOnce({ id: 't1', empresa_id: 'e1' });
      fetchResponde(true, {
        item_id: 'item-1', empresa_id: 'e1', tipo: 'producto', codigo: 'P1',
        nombre: 'P1', precio: '10.00', stock: 0, track_inventory: true,
        duration_minutes: null, atributos: {}, estado: 'activo', source_type: 'producto', source_id: 'p1',
      });
      await expect(servicio.publicarItem('v1', 't1', 'item-1', 10)).rejects.toThrow(/sin stock/);
    });

    it('rechaza un item ya publicado en la tienda', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ id: 't1', empresa_id: 'e1' })
        .mockResolvedValueOnce({ id: 'of-existente' });
      fetchResponde(true, {
        item_id: 'item-1', empresa_id: 'e1', tipo: 'producto', codigo: 'P1',
        nombre: 'P1', precio: '10.00', stock: 5, track_inventory: true,
        duration_minutes: null, atributos: {}, estado: 'activo', source_type: 'producto', source_id: 'p1',
      });
      await expect(servicio.publicarItem('v1', 't1', 'item-1', 10)).rejects.toThrow(/ya esta publicado/);
    });
  });

  describe('sincronizarStockDeOferta (RN-02)', () => {
    it('debe contar las ofertas activas de un producto', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ n: 2 });
      await expect(servicio.contarActivasPorProducto('p1')).resolves.toBe(2);
      expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('COUNT(*)'), ['p1']);
    });

    it('debe recortar el stock negativo a cero y marcar la oferta agotada', async () => {
      const { servicio, pg } = crear();
      await servicio.sincronizarStockDeOferta('e-1', 'SKU-01', -3);
      expect(pg.query).toHaveBeenCalledWith(expect.stringContaining("CASE WHEN $3 > 0"), [
        'e-1',
        'SKU-01',
        0,
      ]);
    });
  });

  describe('consultas', () => {
    it('debe devolver la oferta serializada cuando existe por id', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaOferta);
      const oferta = await servicio.encontrarPorId('of1');
      expect(oferta).toMatchObject({ id: 'of1', empresa_id: 'e1', participacion_porcentaje: 25 });
      expect(oferta?.precio_venta).toBe('125.00');
    });

    it('debe devolver null cuando la oferta no existe por id', async () => {
      const { servicio } = crear();
      await expect(servicio.encontrarPorId('of404')).resolves.toBeNull();
    });

    it('debe devolver vacio sin consultar cuando no hay ids (porIds)', async () => {
      const { servicio, pg } = crear();
      expect(await servicio.porIds([])).toEqual([]);
      expect(pg.query).not.toHaveBeenCalled();
    });

    it('debe consultar ofertas por lote cuando hay ids (porIds)', async () => {
      const { servicio, pg } = crear();
      (pg.query as Mock).mockResolvedValue([filaOferta]);
      await expect(servicio.porIds(['of1'])).resolves.toHaveLength(1);
      expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('ANY($1)'), [['of1']]);
    });

    it('debe paginar las ofertas del vendedor (deVendedor)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ n: 1 });
      (pg.query as Mock).mockResolvedValue([filaOferta]);
      const pagina = await servicio.deVendedor('v1', { pagina: '1', limite: '20' });
      expect(pagina.total).toBe(1);
      expect(pagina.items).toHaveLength(1);
      expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('t.vendedor_id = $1'), ['v1']);
    });

    it('debe listar las ofertas activas de una tienda (deTienda)', async () => {
      const { servicio, pg } = crear();
      (pg.query as Mock).mockResolvedValue([filaOferta]);
      await expect(servicio.deTienda('t1')).resolves.toHaveLength(1);
      expect(pg.query).toHaveBeenCalledWith(expect.stringContaining("o.estado = 'activa'"), ['t1']);
    });
  });
});
