import { ProductosService, Producto } from './productos.service';
import { DomainError, ConflictError, NotFoundError } from '@core/shared';
import type { PoolClient } from 'pg';

/**
 * Catalogo maestro: validacion de SKU/stock/precio (H-03), reserva atomica
 * de stock con bloqueo pesimista (RN-03), devoluciones (RN-06) e
 * idempotencia de eventos (doc 5.2). PgService siempre mockeado (R-QA-2).
 */
describe('ProductosService (catalogo maestro y stock)', () => {
  const crear = () => {
    const client = {
      query: jest.fn().mockResolvedValue({ rows: [], rowCount: 0 }),
    } as unknown as PoolClient;
    const pg = {
      query: jest.fn().mockResolvedValue([]),
      queryOne: jest.fn().mockResolvedValue(null),
      transaccion: jest.fn(async (fn: (c: PoolClient) => Promise<unknown>) => fn(client)),
    };
    const servicio = new ProductosService(pg as never);
    return { servicio, pg, client };
  };

  const filaProducto = {
    id: '11111111-2222-3333-4444-555555555555',
    sku: 'SKU-01',
    nombre: 'Teclado',
    descripcion: 'mecanico',
    categoria: 'perifericos',
    precio_base_cents: 150000,
    stock: 10,
    estado: 'disponible',
    creado_en: '2026-09-01T00:00:00.000Z',
  };

  const datosValidos = {
    sku: 'sku-01',
    nombre: 'Teclado',
    precio_base: '1500.00',
    stock: 10,
  };

  describe('crear (H-03 / TC-01)', () => {
    it('debe rechazar el SKU cuando contiene caracteres fuera de letras, numeros y guiones', async () => {
      const { servicio } = crear();
      await expect(servicio.crear({ ...datosValidos, sku: 'SKU@01' })).rejects.toThrow(DomainError);
      await expect(servicio.crear({ ...datosValidos, sku: 'A' })).rejects.toThrow(/SKU_INVALIDO|letras, numeros/);
    });

    it('debe rechazar el stock cuando es negativo o no entero', async () => {
      const { servicio } = crear();
      await expect(servicio.crear({ ...datosValidos, stock: -1 })).rejects.toThrow(DomainError);
      await expect(servicio.crear({ ...datosValidos, stock: 2.5 })).rejects.toThrow(/stock/i);
    });

    it('debe rechazar el precio cuando no es mayor a cero', async () => {
      const { servicio } = crear();
      await expect(servicio.crear({ ...datosValidos, precio_base: '0' })).rejects.toThrow(/precio/i);
    });

    it('debe devolver ConflictError cuando el SKU ya existe en el catalogo', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(filaProducto);
      await expect(servicio.crear(datosValidos)).rejects.toThrow(ConflictError);
      // el alta no llega a la transaccion
      expect(pg.transaccion).not.toHaveBeenCalled();
    });

    it('debe normalizar el SKU a mayusculas y registrar historico de precio cuando el alta es valida', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValueOnce(null); // encontrarPorSku: no existe
      (client.query as jest.Mock)
        .mockResolvedValueOnce({ rows: [filaProducto], rowCount: 1 }) // INSERT producto
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }); // INSERT historico

      const resultado = await servicio.crear(datosValidos);

      expect(resultado.sku).toBe('SKU-01'); // trim + upper
      const [sqlInsert, params] = (client.query as jest.Mock).mock.calls[0];
      expect(sqlInsert).toContain('INSERT INTO catalog.productos');
      expect(params[1]).toBe('SKU-01'); // sku normalizado
      expect(params[5]).toBe(150000); // precio en centavos (A02)
      const sqlHistorico = (client.query as jest.Mock).mock.calls[1][0];
      expect(sqlHistorico).toContain('INSERT INTO catalog.historico_precios');
    });
  });

  describe('encontrarPorId', () => {
    it('debe devolver null sin consultar la BD cuando el id no es un UUID', async () => {
      const { servicio, pg } = crear();
      expect(await servicio.encontrarPorId('no-es-uuid')).toBeNull();
      expect(pg.queryOne).not.toHaveBeenCalled();
    });

    it('debe serializar el precio a centavos cuando encuentra la fila', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(filaProducto);
      const producto = await servicio.encontrarPorId(filaProducto.id);
      expect(producto).not.toBeNull();
      expect((producto as Producto).precio_base).toBe('1500.00');
      expect((producto as Producto).estado).toBe('disponible');
    });
  });

  describe('consultas por lote', () => {
    it('debe devolver una lista vacia sin consultar cuando no hay SKUs (lotePorSkus)', async () => {
      const { servicio, pg } = crear();
      expect(await servicio.lotePorSkus([])).toEqual([]);
      expect(pg.query).not.toHaveBeenCalled();
    });

    it('debe devolver una lista vacia sin consultar cuando no hay SKUs (stockActualDe)', async () => {
      const { servicio, pg } = crear();
      expect(await servicio.stockActualDe([])).toEqual([]);
      expect(pg.query).not.toHaveBeenCalled();
    });
  });

  describe('reservarStock (RN-03)', () => {
    const ordenOk = async (servicio: ProductosService, pg: any, client: any) => {
      (client.query as jest.Mock)
        .mockResolvedValueOnce({ rowCount: 1, rows: [{ stock: 10 }] }) // SELECT FOR UPDATE
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }) // UPDATE stock
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }); // INSERT reserva
      (pg.query as jest.Mock).mockResolvedValue([{ sku: 'sku-01', stock_restante: 9 }]);
      return servicio.reservarStock('o1', [{ sku: 'sku-01', cantidad: 1 }]);
    };

    it('debe descontar stock y registrar la reserva cuando todo alcanza', async () => {
      const { servicio, pg, client } = crear();
      const resultado = await ordenOk(servicio, pg, client);

      expect(resultado.ok).toBe(true);
      expect(resultado.fallidos).toEqual([]);
      expect(resultado.stock_restante).toEqual([{ sku: 'sku-01', stock_restante: 9 }]);
      const sqlLock = (client.query as jest.Mock).mock.calls[0][0];
      expect(sqlLock).toContain('FOR UPDATE'); // bloqueo pesimista exigido por regla
      const sqlReserva = (client.query as jest.Mock).mock.calls[2][0];
      expect(sqlReserva).toContain('INSERT INTO catalog.reservas_ordenes');
    });

    it('debe rechazar la reserva completa cuando alguna linea no alcanza stock', async () => {
      const { servicio, client } = crear();
      (client.query as jest.Mock).mockResolvedValueOnce({ rowCount: 0, rows: [] }); // sin stock

      const resultado = await servicio.reservarStock('o2', [{ sku: 'agotado', cantidad: 3 }]);

      expect(resultado.ok).toBe(false);
      expect(resultado.fallidos).toEqual([
        { sku: 'agotado', cantidad: 3, motivo: 'stock_insuficiente' },
      ]);
      // jamas inserta la reserva cuando algo fallo
      const sqls = (client.query as jest.Mock).mock.calls.map((c: any[]) => c[0] as string);
      expect(sqls.some((s: string) => s.includes('reservas_ordenes'))).toBe(false);
    });

    it('debe propagar el error cuando la transaccion falla por otra razon', async () => {
      const { servicio, pg } = crear();
      (pg.transaccion as jest.Mock).mockRejectedValueOnce(new Error('conexion caida'));
      await expect(servicio.reservarStock('o3', [])).rejects.toThrow('conexion caida');
    });
  });

  describe('reintegrarStock (RN-06)', () => {
    it('debe sumar stock por cada item y dejar rastro de ajuste cuando se aprueba la devolucion', async () => {
      const { servicio, client } = crear();
      await servicio.reintegrarStock('o1', [
        { sku: 'a', cantidad: 2 },
        { sku: 'b', cantidad: 1 },
      ]);
      const sqls = (client.query as jest.Mock).mock.calls.map((c: any[]) => c[0] as string);
      expect(sqls.filter((s: string) => s.includes('SET stock = stock +')).length).toBe(2);
      expect(sqls.some((s: string) => s.includes('INSERT INTO catalog.ajustes_stock'))).toBe(true);
    });
  });

  describe('idempotencia de eventos (doc 5.2)', () => {
    it('debe responder true cuando el event_id ya fue procesado', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue({ '?column?': 1 });
      expect(await servicio.estaProcesado('evt-1')).toBe(true);
    });

    it('debe responder false cuando el event_id es nuevo', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(null);
      expect(await servicio.estaProcesado('evt-2')).toBe(false);
    });

    it('debe registrar el evento con ON CONFLICT DO NOTHING (duplicados inocuos)', async () => {
      const { servicio, pg } = crear();
      await servicio.registrarProcesado('evt-3', 'order.created');
      const [sql, params] = (pg.query as jest.Mock).mock.calls[0];
      expect(sql).toContain('ON CONFLICT (event_id) DO NOTHING');
      expect(params).toEqual(['evt-3', 'order.created']);
    });
  });

  describe('actualizar (H-03 / RN-08)', () => {
    it('debe lanzar NotFoundError cuando el producto no existe', async () => {
      const { servicio } = crear();
      await expect(servicio.actualizar('no-uuid', { nombre: 'x' })).rejects.toThrow(NotFoundError);
    });

    it('debe rechazar el cambio cuando el nuevo precio no es positivo', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(filaProducto);
      await expect(
        servicio.actualizar(filaProducto.id, { precio_base: '0' }),
      ).rejects.toThrow(/precio/i);
    });

    it('debe rechazar el cambio cuando el stock nuevo no es entero >= 0', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(filaProducto);
      await expect(servicio.actualizar(filaProducto.id, { stock: -2 })).rejects.toThrow(/stock/i);
    });

    it('debe auditar el ajuste de stock y devolver el producto actualizado cuando el cambio es valido', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as jest.Mock)
        .mockResolvedValueOnce(filaProducto) // previo
        .mockResolvedValueOnce({ ...filaProducto, stock: 25 }); // actualizado
      (client.query as jest.Mock).mockResolvedValue({ rows: [], rowCount: 1 });

      const resultado = await servicio.actualizar(filaProducto.id, {
        stock: 25,
        motivo: 'inventario fisico',
      });

      expect(resultado.stock).toBe(25);
      const sqls = (client.query as jest.Mock).mock.calls.map((c: any[]) => c[0] as string);
      expect(sqls.some((s: string) => s.includes('INSERT INTO catalog.ajustes_stock'))).toBe(true);
      expect(sqls.some((s: string) => s.includes('UPDATE catalog.productos'))).toBe(true);
    });
  });

  describe('resumenInventario', () => {
    it('debe devolver ceros cuando la vista no devuelve filas', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(null);
      expect(await servicio.resumenInventario()).toEqual({
        productos: 0,
        agotados: 0,
        stock_total: 0,
        valor_inventario_cents: 0,
      });
    });
  });

  describe('listar', () => {
    it('debe filtrar por texto, estado y categoria cuando llegan los filtros', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue({ n: 0 });
      await servicio.listar({ q: 'Tecl', estado: 'disponible', categoria: 'Perifericos' });

      const [sqlCount, params] = (pg.queryOne as jest.Mock).mock.calls[0];
      expect(sqlCount).toContain('WHERE');
      expect(sqlCount).toContain('LOWER(nombre) LIKE');
      expect(params).toEqual(['%tecl%', 'disponible', 'Perifericos']);
    });

    it('debe calcular offset con la pagina y el limite cuando pagina dos veces', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue({ n: 0 });
      await servicio.listar({ pagina: 3, limite: 5 });

      const [sqlRows, params] = (pg.query as jest.Mock).mock.calls[0];
      expect(sqlRows).not.toContain('WHERE');
      expect(params.slice(-2)).toEqual([5, 10]); // limite 5, offset (3-1)*5
    });
  });
});
