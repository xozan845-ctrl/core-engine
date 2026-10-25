import type { Mock } from 'vitest';
import { ProductosService, Producto } from './productos.service';
import { DomainError, ConflictError, NotFoundError, EMPRESA_LEGACY_ID } from '@core/shared';
import type { PoolClient } from 'pg';

/**
 * Catalogo maestro por empresa (ADR-27): validacion de SKU/stock/precio (H-03),
 * reserva atomica de stock con bloqueo pesimista (RN-03), devoluciones (RN-06)
 * e idempotencia de eventos (doc 5.2). PgService siempre mockeado (R-QA-2).
 */
describe('ProductosService (catalogo maestro y stock)', () => {
  const crear = () => {
    const client = {
      query: vi.fn().mockResolvedValue({ rows: [], rowCount: 0 }),
    } as unknown as PoolClient;
    const pg = {
      query: vi.fn().mockResolvedValue([]),
      queryOne: vi.fn().mockResolvedValue(null),
      transaccion: vi.fn(async (fn: (c: PoolClient) => Promise<unknown>) => fn(client)),
    };
    const servicio = new ProductosService(pg as never);
    return { servicio, pg, client };
  };

  beforeEach(() => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({ estado: 'activa', tipo_empresa: 'comercial' }),
      })),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const filaProducto = {
    id: '11111111-2222-3333-4444-555555555555',
    empresa_id: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
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
    empresa_id: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
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
      (pg.queryOne as Mock).mockResolvedValue(filaProducto);
      await expect(servicio.crear(datosValidos)).rejects.toThrow(ConflictError);
      // el alta no llega a la transaccion
      expect(pg.transaccion).not.toHaveBeenCalled();
    });

    it('debe rechazar la empresa comodin del catalogo legado (ADR-27)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(null);
      await expect(
        servicio.crear({ ...datosValidos, empresa_id: EMPRESA_LEGACY_ID }),
      ).rejects.toThrow(/legado|EMPRESA_NO_PUBLICA/);
      expect(globalThis.fetch).not.toHaveBeenCalled();
    });

    it('debe rechazar una empresa que no es comercial (logistica o plataforma)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(null);
      vi.stubGlobal(
        'fetch',
        vi.fn(async () => ({
          ok: true,
          status: 200,
          json: async () => ({ estado: 'activa', tipo_empresa: 'logistica' }),
        })),
      );
      await expect(servicio.crear(datosValidos)).rejects.toThrow(/comerciales|EMPRESA_SIN_CATALOGO/);
    });

    it('debe normalizar el SKU a mayusculas y registrar historico de precio cuando el alta es valida', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(null) // skuExiste: no existe
        .mockResolvedValue(filaProducto); // detalle posterior al alta
      (client.query as Mock)
        .mockResolvedValueOnce({ rows: [filaProducto], rowCount: 1 }) // INSERT producto
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }); // INSERT historico

      const resultado = await servicio.crear(datosValidos);

      expect(resultado.sku).toBe('SKU-01'); // trim + upper
      const [sqlInsert, params] = (client.query as Mock).mock.calls[0];
      expect(sqlInsert).toContain('INSERT INTO catalog.productos');
      expect(params[2]).toBe('SKU-01'); // sku normalizado
      expect(params[6]).toBe(150000); // precio en centavos (A02)
      const sqlHistorico = (client.query as Mock).mock.calls[1][0];
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
      (pg.queryOne as Mock).mockResolvedValue(filaProducto);
      const producto = await servicio.encontrarPorId(filaProducto.id);
      expect(producto).not.toBeNull();
      expect((producto as Producto).precio_base).toBe('1500.00');
      expect((producto as Producto).estado).toBe('disponible');
    });

    it('debe serializar como agotado cuando la fila no esta disponible', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ ...filaProducto, estado: 'agotado' });
      const producto = await servicio.encontrarPorId(filaProducto.id);
      expect((producto as Producto).estado).toBe('agotado');
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

    it('debe consultar por lote cuando hay SKUs (lotePorSkus)', async () => {
      const { servicio, pg } = crear();
      (pg.query as Mock).mockResolvedValue([{ sku: 'sku-01', stock: 9, estado: 'disponible' }]);
      await expect(servicio.lotePorSkus(['SKU-01', 'sku-01'])).resolves.toEqual([
        { sku: 'sku-01', stock: 9, estado: 'disponible' },
      ]);
      const [sql, params] = (pg.query as Mock).mock.calls[0];
      expect(sql).toContain('LOWER(sku) IN');
      expect(params).toEqual(['sku-01']); // deduplicado y en minusculas
    });

    it('debe consultar el stock actual cuando hay productos (stockActualDe)', async () => {
      const { servicio, pg } = crear();
      (pg.query as Mock).mockResolvedValue([{ empresa_id: datosValidos.empresa_id, sku: 'sku-01', stock_restante: 9 }]);
      await expect(
        servicio.stockActualDe([{ empresa_id: datosValidos.empresa_id, sku: 'SKU-01' }]),
      ).resolves.toEqual([{ empresa_id: datosValidos.empresa_id, sku: 'sku-01', stock_restante: 9 }]);
      const [sql, params] = (pg.query as Mock).mock.calls[0];
      expect(sql).toContain('stock_restante');
      expect(JSON.parse(params[0] as string)).toEqual([
        { empresa_id: datosValidos.empresa_id, sku: 'sku-01' },
      ]);
    });
  });

  describe('reservarStock (RN-03)', () => {
    const ordenOk = async (servicio: ProductosService, pg: any, client: any) => {
      (client.query as Mock)
        .mockResolvedValueOnce({ rowCount: 1, rows: [{ stock: 10 }] }) // SELECT FOR UPDATE
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }) // UPDATE stock
        .mockResolvedValueOnce({ rows: [], rowCount: 1 }); // INSERT reserva
      (pg.query as Mock).mockResolvedValue([{ empresa_id: datosValidos.empresa_id, sku: 'sku-01', stock_restante: 9 }]);
      return servicio.reservarStock('o1', [
        { empresa_id: datosValidos.empresa_id, sku: 'sku-01', cantidad: 1 },
      ]);
    };

    it('debe descontar stock y registrar la reserva cuando todo alcanza', async () => {
      const { servicio, pg, client } = crear();
      const resultado = await ordenOk(servicio, pg, client);

      expect(resultado.ok).toBe(true);
      expect(resultado.fallidos).toEqual([]);
      expect(resultado.stock_restante).toEqual([
        { empresa_id: datosValidos.empresa_id, sku: 'sku-01', stock_restante: 9 },
      ]);
      const sqlLock = (client.query as Mock).mock.calls[0][0];
      expect(sqlLock).toContain('FOR UPDATE'); // bloqueo pesimista exigido por regla
      expect(sqlLock).toContain('empresa_id = $1');
      const sqlReserva = (client.query as Mock).mock.calls[2][0];
      expect(sqlReserva).toContain('INSERT INTO catalog.reservas_ordenes');
    });

    it('debe rechazar la reserva completa cuando alguna linea no alcanza stock', async () => {
      const { servicio, client } = crear();
      (client.query as Mock).mockResolvedValueOnce({ rowCount: 0, rows: [] }); // sin stock

      const resultado = await servicio.reservarStock('o2', [
        { empresa_id: datosValidos.empresa_id, sku: 'agotado', cantidad: 3 },
      ]);

      expect(resultado.ok).toBe(false);
      expect(resultado.fallidos).toEqual([
        {
          empresa_id: datosValidos.empresa_id,
          sku: 'agotado',
          cantidad: 3,
          motivo: 'stock_insuficiente',
        },
      ]);
      // jamas inserta la reserva cuando algo fallo
      const sqls = (client.query as Mock).mock.calls.map((c: any[]) => c[0] as string);
      expect(sqls.some((s: string) => s.includes('reservas_ordenes'))).toBe(false);
    });

    it('debe propagar el error cuando la transaccion falla por otra razon', async () => {
      const { servicio, pg } = crear();
      (pg.transaccion as Mock).mockRejectedValueOnce(new Error('conexion caida'));
      await expect(servicio.reservarStock('o3', [])).rejects.toThrow('conexion caida');
    });
  });

  describe('reintegrarStock (RN-06)', () => {
    it('debe sumar stock por cada item y dejar rastro de ajuste cuando se aprueba la devolucion', async () => {
      const { servicio, client } = crear();
      await servicio.reintegrarStock('o1', [
        { empresa_id: datosValidos.empresa_id, sku: 'a', cantidad: 2 },
        { empresa_id: datosValidos.empresa_id, sku: 'b', cantidad: 1 },
      ]);
      const sqls = (client.query as Mock).mock.calls.map((c: any[]) => c[0] as string);
      expect(sqls.filter((s: string) => s.includes('SET stock = stock +')).length).toBe(2);
      expect(sqls.some((s: string) => s.includes('INSERT INTO catalog.ajustes_stock'))).toBe(true);
      expect(sqls.some((s: string) => s.includes('DELETE FROM catalog.reservas_ordenes'))).toBe(true);
    });
  });

  describe('idempotencia de eventos (doc 5.2)', () => {
    it('debe responder true cuando el event_id ya fue procesado', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ '?column?': 1 });
      expect(await servicio.estaProcesado('evt-1')).toBe(true);
    });

    it('debe responder false cuando el event_id es nuevo', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(null);
      expect(await servicio.estaProcesado('evt-2')).toBe(false);
    });

    it('debe registrar el evento con ON CONFLICT DO NOTHING (duplicados inocuos)', async () => {
      const { servicio, pg } = crear();
      await servicio.registrarProcesado('evt-3', 'order.created');
      const [sql, params] = (pg.query as Mock).mock.calls[0];
      expect(sql).toContain('ON CONFLICT (event_id) DO NOTHING');
      expect(params).toEqual(['evt-3', 'order.created']);
    });
  });

  describe('actualizar (H-03 / RN-08)', () => {
    it('oculta el producto cuando pertenece a otra empresa', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(null);
      await expect(servicio.actualizar(filaProducto.id, { nombre: 'Otro' }, 'otra-empresa'))
        .rejects.toThrow(NotFoundError);
      const [sql, params] = (pg.queryOne as Mock).mock.calls[0];
      expect(sql).toContain('empresa_id');
      expect(params).toEqual([filaProducto.id, 'otra-empresa']);
    });

    it('debe lanzar NotFoundError cuando el producto no existe', async () => {
      const { servicio } = crear();
      await expect(servicio.actualizar('no-uuid', { nombre: 'x' })).rejects.toThrow(NotFoundError);
    });

    it('debe rechazar el cambio cuando el nuevo precio no es positivo', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaProducto);
      await expect(
        servicio.actualizar(filaProducto.id, { precio_base: '0' }),
      ).rejects.toThrow(/precio/i);
    });

    it('debe rechazar el cambio cuando el stock nuevo no es entero >= 0', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaProducto);
      await expect(servicio.actualizar(filaProducto.id, { stock: -2 })).rejects.toThrow(/stock/i);
    });

    it('debe auditar el ajuste de stock y devolver el producto actualizado cuando el cambio es valido', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(filaProducto) // previo
        .mockResolvedValueOnce({ ...filaProducto, stock: 25 }); // actualizado
      (client.query as Mock).mockResolvedValue({ rows: [], rowCount: 1 });

      const resultado = await servicio.actualizar(filaProducto.id, {
        stock: 25,
        motivo: 'inventario fisico',
      });

      expect(resultado.stock).toBe(25);
      const sqls = (client.query as Mock).mock.calls.map((c: any[]) => c[0] as string);
      expect(sqls.some((s: string) => s.includes('INSERT INTO catalog.ajustes_stock'))).toBe(true);
      expect(sqls.some((s: string) => s.includes('UPDATE catalog.productos'))).toBe(true);
    });

    it('debe registrar el historico cuando cambia el precio base (RN-08)', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(filaProducto)
        .mockResolvedValueOnce({ ...filaProducto, precio_base_cents: 200000 });
      (client.query as Mock).mockResolvedValue({ rows: [], rowCount: 1 });

      const resultado = await servicio.actualizar(filaProducto.id, { precio_base: '2000.00' });

      expect(resultado.precio_base).toBe('2000.00');
      const [sqlHistorico, paramsHistorico] = (client.query as Mock).mock.calls.find(
        (c: any[]) => String(c[0]).includes('historico_precios'),
      ) as [string, unknown[]];
      expect(sqlHistorico).toContain('historico_precios');
      expect(paramsHistorico).toEqual([
        filaProducto.empresa_id,
        'SKU-01',
        150000,
        200000,
      ]);
    });

    it('debe actualizar nombre, descripcion y categoria cuando llegan esos campos', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(filaProducto)
        .mockResolvedValueOnce({ ...filaProducto, nombre: 'Nuevo', descripcion: 'D', categoria: 'C' });
      (client.query as Mock).mockResolvedValue({ rows: [], rowCount: 1 });

      await servicio.actualizar(filaProducto.id, {
        nombre: 'Nuevo',
        descripcion: 'D',
        categoria: 'C',
      });

      const [sqlUpdate, params] = (client.query as Mock).mock.calls.find(
        (c: any[]) => String(c[0]).includes('UPDATE catalog.productos'),
      ) as [string, unknown[]];
      expect(sqlUpdate).toContain('nombre = $1');
      expect(sqlUpdate).toContain('descripcion = $2');
      expect(sqlUpdate).toContain('categoria = $3');
      expect(params).toEqual(['Nuevo', 'D', 'C', filaProducto.id]);
    });
  });

  describe('resumenInventario', () => {
    it('debe devolver ceros cuando la vista no devuelve filas', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(null);
      expect(await servicio.resumenInventario()).toEqual({
        productos: 0,
        agotados: 0,
        stock_total: 0,
        valor_inventario_cents: 0,
      });
    });

    it('debe devolver el resumen cuando la vista devuelve agregados', async () => {
      const { servicio, pg } = crear();
      const resumen = { productos: 2, agotados: 1, stock_total: 7, valor_inventario_cents: 7000 };
      (pg.queryOne as Mock).mockResolvedValue(resumen);
      await expect(servicio.resumenInventario()).resolves.toEqual(resumen);
    });
  });

  describe('listar', () => {
    it('debe filtrar por texto, estado y categoria cuando llegan los filtros', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ n: 0 });
      await servicio.listar({
        empresa_id: datosValidos.empresa_id,
        q: 'Tecl',
        estado: 'disponible',
        categoria: 'Perifericos',
      });

      const [sqlCount, params] = (pg.queryOne as Mock).mock.calls[0];
      expect(sqlCount).toContain('WHERE');
      expect(sqlCount).toContain('empresa_id = $1');
      expect(sqlCount).toContain('LOWER(v.nombre) LIKE');
      expect(params).toEqual([datosValidos.empresa_id, '%tecl%', 'disponible', 'Perifericos']);
    });

    it('debe calcular offset con la pagina y el limite cuando pagina dos veces', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ n: 0 });
      await servicio.listar({
        empresa_id: datosValidos.empresa_id,
        pagina: 3,
        limite: 5,
      });

      const [sqlRows, params] = (pg.query as Mock).mock.calls[0];
      expect(sqlRows).toContain('empresa_id = $1');
      expect(params.slice(-2)).toEqual([5, 10]); // limite 5, offset (3-1)*5
    });

    it('debe exigir empresa y nunca listar el catalogo legado (ADR-27)', async () => {
      const { servicio, pg } = crear();
      await expect(servicio.listar({})).rejects.toThrow(/empresa_id|EMPRESA_REQUERIDA/);
      await expect(servicio.listar({ empresa_id: EMPRESA_LEGACY_ID })).rejects.toThrow(
        /legado|EMPRESA_NO_PUBLICA/,
      );
      expect(pg.query).not.toHaveBeenCalled();
      expect(pg.queryOne).not.toHaveBeenCalled();
    });
  });

  describe('variantes e imágenes (ADR-28)', () => {
    const VARIANTE_ID = '99999999-8888-4777-8666-555555555555';
    const IMAGEN_ID = '33333333-4444-4555-8666-777777777777';
    const OBJETO_ID = '22222222-3333-4444-8555-666666666666';
    const filaVariante = {
      ...filaProducto,
      id: VARIANTE_ID,
      sku: 'SKU-01-ROJO',
      producto_padre_id: filaProducto.id,
      atributos: { color: 'rojo' },
      tiene_variantes: false,
    };
    const filaImagen = {
      id: IMAGEN_ID,
      producto_id: filaProducto.id,
      objeto_id: OBJETO_ID,
      orden: 0,
      es_principal: true,
    };

    const fetchCatalogos = (publico = true) =>
      vi.stubGlobal(
        'fetch',
        vi.fn(async (url: string | URL | Request) => {
          if (String(url).includes('/internal/empresas/')) {
            return {
              ok: true,
              status: 200,
              json: async () => ({ estado: 'activa', tipo_empresa: 'comercial' }),
            } as Response;
          }
          return {
            ok: true,
            status: 200,
            json: async () => ({
              tenant_id: datosValidos.empresa_id,
              estado: 'activo',
              content_type: 'image/png',
              publico,
            }),
          } as Response;
        }),
      );

    it('debe crear el padre con variantes e imágenes y devolver el detalle', async () => {
      const { servicio, pg, client } = crear();
      fetchCatalogos();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(null) // sku padre libre
        .mockResolvedValueOnce(null) // sku variante libre
        .mockResolvedValue({ ...filaProducto, tiene_variantes: true }); // detalle
      (pg.query as Mock)
        .mockResolvedValueOnce([filaVariante]) // hijos
        .mockResolvedValueOnce([filaImagen]); // imágenes
      (client.query as Mock).mockResolvedValue({ rows: [], rowCount: 1 });

      const producto = await servicio.crear({
        ...datosValidos,
        variantes: [
          {
            sku: 'SKU-01-ROJO',
            nombre: 'Teclado rojo',
            atributos: { color: 'rojo' },
            precio_base: '1600.00',
            stock: 3,
          },
        ],
        imagenes: [{ objeto_id: OBJETO_ID, es_principal: true }],
      });

      expect(producto.tiene_variantes).toBe(true);
      expect(producto.variantes).toHaveLength(1);
      expect(producto.variantes?.[0].sku).toBe('SKU-01-ROJO');
      expect(producto.imagenes[0].url).toBe(`/api/v1/storage/public/objetos/${OBJETO_ID}`);
      const sqls = (client.query as Mock).mock.calls.map((c) => String(c[0]));
      expect(sqls.some((s) => s.includes('producto_padre_id'))).toBe(true);
      expect(sqls.some((s) => s.includes('fn_recalcular_padre'))).toBe(true);
    });

    it('debe rechazar una imagen que no está marcada como pública', async () => {
      const { servicio, pg } = crear();
      fetchCatalogos(false);
      (pg.queryOne as Mock).mockResolvedValueOnce(null);
      await expect(
        servicio.crear({
          ...datosValidos,
          imagenes: [{ objeto_id: OBJETO_ID }],
        }),
      ).rejects.toThrow(/publico|IMAGEN_NO_PUBLICA/);
      expect(pg.transaccion).not.toHaveBeenCalled();
    });

    it('debe rechazar más de tres imágenes', async () => {
      const { servicio } = crear();
      await expect(
        servicio.crear({
          ...datosValidos,
          imagenes: [0, 1, 2, 3].map((i) => ({ objeto_id: `${i}${OBJETO_ID.slice(1)}` })),
        }),
      ).rejects.toThrow(/3 imágenes|LIMITE_IMAGENES/);
    });

    it('debe listar solo padres y adjuntar sus imágenes', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ n: 1 });
      (pg.query as Mock)
        .mockResolvedValueOnce([filaProducto])
        .mockResolvedValueOnce([filaImagen]);

      const pagina = await servicio.listar({ empresa_id: datosValidos.empresa_id });

      expect(pagina.items[0].imagenes).toHaveLength(1);
      expect(pagina.items[0].imagenes[0].es_principal).toBe(true);
      const [sqlRows] = (pg.query as Mock).mock.calls[0];
      expect(sqlRows).toContain('producto_padre_id IS NULL');
    });

    it('debe impedir reservar stock del padre si tiene variantes', async () => {
      const { servicio, client } = crear();
      (client.query as Mock).mockResolvedValueOnce({
        rowCount: 1,
        rows: [{ stock: 10, tiene_variantes: true }],
      });

      const resultado = await servicio.reservarStock('o1', [
        { empresa_id: datosValidos.empresa_id, sku: 'SKU-01', cantidad: 1 },
      ]);

      expect(resultado.ok).toBe(false);
      expect(resultado.fallidos[0].motivo).toBe('variante_requerida');
    });

    it('debe impedir editar precio/stock del padre con variantes', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ ...filaProducto, tiene_variantes: true });
      (pg.query as Mock).mockResolvedValue([]);

      await expect(
        servicio.actualizar(filaProducto.id, { stock: 5 }, datosValidos.empresa_id),
      ).rejects.toThrow(/variantes/i);
    });
  });

  describe('gestión de variantes e imágenes', () => {
    const VARIANTE_ID = '99999999-8888-4777-8666-555555555555';
    const IMAGEN_ID = '33333333-4444-4555-8666-777777777777';
    const OBJETO_ID = '22222222-3333-4444-8555-666666666666';
    const padre = { ...filaProducto, tiene_variantes: true, producto_padre_id: null };
    const variante = {
      ...filaProducto,
      id: VARIANTE_ID,
      sku: 'SKU-01-ROJO',
      producto_padre_id: filaProducto.id,
      tiene_variantes: false,
      atributos: { color: 'rojo' },
    };
    const imagen = {
      id: IMAGEN_ID,
      producto_id: filaProducto.id,
      objeto_id: OBJETO_ID,
      orden: 0,
      es_principal: true,
    };

    const fetchCatalogo = (respuesta: Record<string, unknown> | Error) =>
      vi.stubGlobal(
        'fetch',
        vi.fn(async (url: string | URL | Request) => {
          if (String(url).includes('/internal/empresas/')) {
            return {
              ok: true,
              status: 200,
              json: async () => ({ estado: 'activa', tipo_empresa: 'comercial' }),
            } as Response;
          }
          if (respuesta instanceof Error) throw respuesta;
          return respuesta as Response;
        }),
      );

    it('debe crear una variante y recalcular el padre', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(padre)
        .mockResolvedValueOnce(null)
        .mockResolvedValue(variante);
      (pg.query as Mock).mockResolvedValue([]);
      (client.query as Mock).mockResolvedValue({ rows: [], rowCount: 1 });

      const creada = await servicio.crearVariante(
        filaProducto.id,
        { sku: 'sku-01-rojo', precio_base: '1600.00', stock: 3, atributos: { color: 'rojo' } },
        datosValidos.empresa_id,
      );

      expect(creada.sku).toBe('SKU-01-ROJO');
      const sqls = (client.query as Mock).mock.calls.map((c) => String(c[0]));
      expect(sqls.some((s) => s.includes('INSERT INTO catalog.productos'))).toBe(true);
      expect(sqls.some((s) => s.includes('fn_recalcular_padre'))).toBe(true);
    });

    it('debe rechazar una variante de una variante', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(variante);
      (pg.query as Mock).mockResolvedValue([]);

      await expect(
        servicio.crearVariante(
          VARIANTE_ID,
          { sku: 'SKU-03', precio_base: '10.00', stock: 1 },
          datosValidos.empresa_id,
        ),
      ).rejects.toThrow(/padre|NO_ES_PADRE/);
    });

    it('debe rechazar una variante con SKU duplicado', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...padre, tiene_variantes: false })
        .mockResolvedValueOnce({ id: 'existente' });
      (pg.query as Mock).mockResolvedValue([]);

      await expect(
        servicio.crearVariante(
          filaProducto.id,
          { sku: 'SKU-01', precio_base: '10.00', stock: 1 },
          datosValidos.empresa_id,
        ),
      ).rejects.toThrow(ConflictError);
    });

    it('debe actualizar y eliminar una variante existente', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...padre, tiene_variantes: true })
        .mockResolvedValueOnce(variante) // detalle de la variante al actualizar
        .mockResolvedValue(variante);
      (pg.query as Mock).mockResolvedValueOnce([variante]).mockResolvedValue([]);
      (client.query as Mock).mockResolvedValue({ rows: [], rowCount: 1 });

      await expect(
        servicio.actualizarVariante(
          filaProducto.id,
          VARIANTE_ID,
          { stock: 4 },
          datosValidos.empresa_id,
        ),
      ).resolves.toBeTruthy();

      (pg.queryOne as Mock).mockResolvedValueOnce({ ...padre, tiene_variantes: true });
      (pg.query as Mock).mockResolvedValueOnce([variante]).mockResolvedValue([]);
      await expect(
        servicio.eliminarVariante(filaProducto.id, VARIANTE_ID, datosValidos.empresa_id),
      ).resolves.toBeUndefined();
      expect((client.query as Mock).mock.calls.some((c) => String(c[0]).includes('DELETE FROM catalog.productos'))).toBe(true);
    });

    it('debe rechazar eliminar una variante que no pertenece al padre', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ ...padre, tiene_variantes: true });
      (pg.query as Mock).mockResolvedValueOnce([]).mockResolvedValue([]);

      await expect(
        servicio.eliminarVariante(filaProducto.id, VARIANTE_ID, datosValidos.empresa_id),
      ).rejects.toThrow(NotFoundError);
    });

    it('debe agregar una imagen y promoverla a principal si es la primera', async () => {
      const { servicio, pg, client } = crear();
      fetchCatalogo({
        ok: true,
        status: 200,
        json: async () => ({
          tenant_id: datosValidos.empresa_id,
          estado: 'activo',
          content_type: 'image/png',
          publico: true,
        }),
      });
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...filaProducto, tiene_variantes: false })
        .mockResolvedValue({ ...filaProducto, tiene_variantes: false });
      (pg.query as Mock).mockResolvedValueOnce([]).mockResolvedValueOnce([]).mockResolvedValue([]);
      (client.query as Mock)
        .mockResolvedValueOnce({ rows: [], rowCount: 0 })
        .mockResolvedValue({ rows: [], rowCount: 1 });

      const resultado = await servicio.agregarImagen(
        filaProducto.id,
        { objeto_id: OBJETO_ID },
        datosValidos.empresa_id,
      );

      expect(resultado.id).toBe(filaProducto.id);
      const sqls = (client.query as Mock).mock.calls.map((c) => String(c[0]));
      expect(sqls.some((s) => s.includes('INSERT INTO catalog.producto_imagenes'))).toBe(true);
      expect(sqls.some((s) => s.includes('SET es_principal = true'))).toBe(true);
    });

    it('debe rechazar una cuarta imagen', async () => {
      const { servicio, pg, client } = crear();
      fetchCatalogo({
        ok: true,
        status: 200,
        json: async () => ({
          tenant_id: datosValidos.empresa_id,
          estado: 'activo',
          content_type: 'image/png',
          publico: true,
        }),
      });
      (pg.queryOne as Mock).mockResolvedValueOnce({
        ...filaProducto,
        tiene_variantes: false,
        imagenes: [imagen, { ...imagen, id: 'i-2', orden: 1 }, { ...imagen, id: 'i-3', orden: 2 }],
      });
      (pg.query as Mock).mockResolvedValueOnce([]).mockResolvedValue([imagen]);
      (client.query as Mock).mockResolvedValueOnce({
        rows: [imagen, { ...imagen, id: 'i-2', orden: 1 }, { ...imagen, id: 'i-3', orden: 2 }],
        rowCount: 3,
      });

      await expect(
        servicio.agregarImagen(filaProducto.id, { objeto_id: OBJETO_ID }, datosValidos.empresa_id),
      ).rejects.toThrow(/3 imágenes|LIMITE_IMAGENES/);
    });

    it('debe eliminar una imagen principal y promover la siguiente', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as Mock).mockResolvedValue({
        ...filaProducto,
        tiene_variantes: false,
        imagenes: [imagen],
      });
      (pg.query as Mock).mockResolvedValueOnce([]).mockResolvedValueOnce([imagen]).mockResolvedValue([]);
      (client.query as Mock)
        .mockResolvedValueOnce({ rows: [{ es_principal: true }], rowCount: 1 })
        .mockResolvedValue({ rows: [], rowCount: 1 });

      await servicio.eliminarImagen(filaProducto.id, IMAGEN_ID, datosValidos.empresa_id);
      const sqls = (client.query as Mock).mock.calls.map((c) => String(c[0]));
      expect(sqls.some((s) => s.includes('DELETE FROM catalog.producto_imagenes'))).toBe(true);
      expect(sqls.some((s) => s.includes('SET es_principal = true'))).toBe(true);
    });

    it('debe rechazar eliminar una imagen que no pertenece al producto', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ ...filaProducto, tiene_variantes: false });
      (pg.query as Mock).mockResolvedValueOnce([]).mockResolvedValue([]);

      await expect(
        servicio.eliminarImagen(filaProducto.id, IMAGEN_ID, datosValidos.empresa_id),
      ).rejects.toThrow(NotFoundError);
    });

    it.each([
      ['tenant distinto', { tenant_id: 'otro', estado: 'activo', content_type: 'image/png', publico: true }, /otra empresa/],
      ['inactiva', { tenant_id: datosValidos.empresa_id, estado: 'pendiente', content_type: 'image/png', publico: true }, /activa/],
      ['no imagen', { tenant_id: datosValidos.empresa_id, estado: 'activo', content_type: 'application/pdf', publico: true }, /no es una imagen/],
    ])('debe rechazar la imagen %s', async (_nombre, cuerpo, esperado) => {
      const { servicio, pg } = crear();
      fetchCatalogo({ ok: true, status: 200, json: async () => cuerpo });
      (pg.queryOne as Mock).mockResolvedValueOnce(null);

      await expect(
        servicio.crear({ ...datosValidos, imagenes: [{ objeto_id: OBJETO_ID }] }),
      ).rejects.toThrow(esperado);
      expect(pg.transaccion).not.toHaveBeenCalled();
    });

    it('debe fallar cerrado si storage no responde', async () => {
      const { servicio, pg } = crear();
      fetchCatalogo(new Error('red caida'));
      (pg.queryOne as Mock).mockResolvedValueOnce(null);

      await expect(
        servicio.crear({ ...datosValidos, imagenes: [{ objeto_id: OBJETO_ID }] }),
      ).rejects.toThrow(/verificar la imagen/);
    });
  });

  describe('ramas de validación ADR-28', () => {
    const OBJETO_ID = '22222222-3333-4444-8555-666666666666';

    it('debe encontrar un producto por SKU y devolver su detalle', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaProducto);
      (pg.query as Mock).mockResolvedValue([]);

      const producto = await servicio.encontrarPorSku('SKU-01', datosValidos.empresa_id);

      expect(producto?.sku).toBe('SKU-01');
      expect(producto?.tiene_variantes).toBe(false);
    });

    it('debe rechazar más de 50 variantes en el alta', async () => {
      const { servicio } = crear();
      const variantes = Array.from({ length: 51 }, (_, i) => ({
        sku: `SKU-${i}`,
        precio_base: '10.00',
        stock: 1,
      }));
      await expect(servicio.crear({ ...datosValidos, variantes })).rejects.toThrow(
        /DEMASIADAS_VARIANTES|máximo 50/,
      );
    });

    it('debe rechazar dos variantes con el mismo SKU en la solicitud', async () => {
      const { servicio } = crear();
      await expect(
        servicio.crear({
          ...datosValidos,
          variantes: [
            { sku: 'SKU-02', precio_base: '10.00', stock: 1 },
            { sku: 'SKU-02', precio_base: '11.00', stock: 1 },
          ],
        }),
      ).rejects.toThrow(ConflictError);
    });

    it('debe actualizar atributos sin tocar precio/stock', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaProducto);
      (pg.query as Mock).mockResolvedValue([]);
      (client.query as Mock).mockResolvedValue({ rows: [], rowCount: 1 });

      const producto = await servicio.actualizar(
        filaProducto.id,
        { atributos: { color: 'rojo' } },
        datosValidos.empresa_id,
      );

      expect(producto.id).toBe(filaProducto.id);
      const sqls = (client.query as Mock).mock.calls.map((c) => String(c[0]));
      expect(sqls.some((s) => s.includes('atributos ='))).toBe(true);
    });

    it('debe rechazar una variante que no pertenece al padre', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ ...filaProducto, tiene_variantes: true });
      (pg.query as Mock).mockResolvedValue([]);

      await expect(
        servicio.actualizarVariante(
          filaProducto.id,
          '99999999-8888-4777-8666-555555555555',
          { stock: 2 },
          datosValidos.empresa_id,
        ),
      ).rejects.toThrow(NotFoundError);
    });

    it('debe rechazar un orden de imagen ya usado al agregar', async () => {
      const { servicio, pg, client } = crear();
      vi.stubGlobal(
        'fetch',
        vi.fn(async (url: string | URL | Request) => {
          if (String(url).includes('/internal/empresas/')) {
            return { ok: true, status: 200, json: async () => ({ estado: 'activa', tipo_empresa: 'comercial' }) } as Response;
          }
          return {
            ok: true,
            status: 200,
            json: async () => ({
              tenant_id: datosValidos.empresa_id,
              estado: 'activo',
              content_type: 'image/png',
              publico: true,
            }),
          } as Response;
        }),
      );
      (pg.queryOne as Mock).mockResolvedValue({ ...filaProducto, tiene_variantes: false });
      (pg.query as Mock).mockResolvedValue([]);
      (client.query as Mock).mockResolvedValueOnce({
        rows: [{ id: 'img-1', producto_id: filaProducto.id, objeto_id: OBJETO_ID, orden: 0, es_principal: true }],
        rowCount: 1,
      });

      await expect(
        servicio.agregarImagen(
          filaProducto.id,
          { objeto_id: OBJETO_ID, orden: 0 },
          datosValidos.empresa_id,
        ),
      ).rejects.toThrow(/orden/i);
    });

    it('debe rechazar un objeto de imagen que no es UUID', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(null);

      await expect(
        servicio.crear({ ...datosValidos, imagenes: [{ objeto_id: 'no-uuid' }] }),
      ).rejects.toThrow(/UUID/);
    });

    it('debe fallar si identity no responde al validar la empresa', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(null);
      vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('red caida')));

      await expect(servicio.crear(datosValidos)).rejects.toThrow(/verificar la empresa/);
    });
  });
});
