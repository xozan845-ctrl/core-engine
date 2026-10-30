import type { Mock } from 'vitest';
import { ConflictError, DomainError, NotFoundError } from '@core/shared';
import { OperacionesService } from './operaciones.service';

const EMPRESA = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';
const PRODUCTO = {
  id: '11111111-2222-3333-4444-555555555555',
  sku: 'SKU-01',
  nombre: 'Producto',
  costo_cents: 1000,
  stock: 10,
};
const COMBO_ROW = {
  id: '22222222-2222-3333-4444-555555555555',
  empresa_id: EMPRESA,
  sku: 'COMBO-01',
  nombre: 'Combo',
  descripcion: '',
  codigo_barras: '',
  precio_cents: 3000,
  estado: 'activo',
  componentes: [
    { producto_id: PRODUCTO.id, sku: PRODUCTO.sku, nombre: PRODUCTO.nombre, cantidad: 2 },
  ],
  creado_en: '2026-10-30T10:00:00.000Z',
  actualizado_en: '2026-10-30T10:00:00.000Z',
};
const PROMO_ROW = {
  id: '33333333-2222-3333-4444-555555555555',
  empresa_id: EMPRESA,
  nombre: '2x1',
  tipo: '2x1',
  producto_id: PRODUCTO.id,
  sku: PRODUCTO.sku,
  cantidad_requerida: 2,
  cantidad_gratis: 1,
  descuento_porcentaje: null,
  fecha_desde: '2026-10-01',
  fecha_hasta: '2026-12-31',
  estado: 'activo',
  creado_en: '2026-10-30T10:00:00.000Z',
  actualizado_en: '2026-10-30T10:00:00.000Z',
};
const MOV_ROW = {
  id: '44444444-2222-3333-4444-555555555555',
  empresa_id: EMPRESA,
  tipo: 'entrada',
  motivo: 'compra',
  referencia: '',
  ubicacion_origen: '',
  ubicacion_destino: '',
  lineas: [],
  creado_por: 'u1',
  creado_en: '2026-10-30T10:00:00.000Z',
};

function crear() {
  const client = { query: vi.fn().mockResolvedValue({ rows: [MOV_ROW], rowCount: 1 }) };
  const pg = {
    query: vi.fn().mockResolvedValue([]),
    queryOne: vi.fn().mockResolvedValue(null),
    transaccion: vi.fn(async (fn: (c: unknown) => Promise<unknown>) => fn(client)),
  };
  const productos = {
    validarEmpresaCatalogo: vi.fn().mockResolvedValue(undefined),
    crear: vi.fn().mockResolvedValue({}),
    encontrarPorCodigoBarras: vi.fn().mockResolvedValue(null),
  };
  const servicios = { encontrarPorCodigo: vi.fn().mockResolvedValue(null) };
  const outbox = { insertarEnTransaccion: vi.fn().mockResolvedValue(undefined) };
  const servicio = new OperacionesService(
    pg as never,
    productos as never,
    servicios as never,
    outbox as never,
  );
  return { servicio, pg, client, productos, servicios, outbox };
}

describe('OperacionesService (R-PS-04)', () => {
  it('devuelve la configuración por defecto y guarda overrides', async () => {
    const { servicio, pg } = crear();
    const inicial = await servicio.obtenerConfiguracion(EMPRESA);
    expect(inicial).toMatchObject({ metodo_costeo: 'promedio', permitir_stock_negativo: false });

    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce({
      empresa_id: EMPRESA,
      metodo_costeo: 'fifo',
      permitir_stock_negativo: true,
      alerta_stock_bajo: false,
      actualizar_costo_en_compra: false,
      unidad_defecto: 'Caja',
      actualizado_en: '2026-10-30T10:00:00.000Z',
    }).mockResolvedValueOnce({
      empresa_id: EMPRESA,
      metodo_costeo: 'fifo',
      permitir_stock_negativo: true,
      alerta_stock_bajo: true,
      actualizar_costo_en_compra: true,
      unidad_defecto: 'Unidad (u)',
      actualizado_en: '2026-10-30T10:00:00.000Z',
    });
    const guardada = await servicio.guardarConfiguracion(EMPRESA, {
      metodo_costeo: 'fifo',
      permitir_stock_negativo: true,
    });
    expect(guardada).toMatchObject({ metodo_costeo: 'fifo', permitir_stock_negativo: true });
  });

  it('rechaza un método de costeo inválido', async () => {
    const { servicio } = crear();
    await expect(
      servicio.guardarConfiguracion(EMPRESA, { metodo_costeo: 'otro' as never }),
    ).rejects.toMatchObject({ codigo: 'METODO_COSTEO_INVALIDO' });
  });

  it('importa productos y reporta omitidos/errores', async () => {
    const { servicio, productos } = crear();
    productos.crear
      .mockResolvedValueOnce({})
      .mockRejectedValueOnce(new ConflictError('duplicado'))
      .mockRejectedValueOnce(new DomainError('SKU_INVALIDO', 'sku inválido'));

    const resultado = await servicio.importarProductos(EMPRESA, [
      { sku: 'A-1', nombre: 'A', precio_base: '10.00', stock: 1 },
      { sku: 'A-2', nombre: 'B', precio_base: '10.00', stock: 1 },
      { sku: 'A-3', nombre: 'C', precio_base: '10.00', stock: 1 },
    ]);

    expect(resultado).toMatchObject({ importados: 1, omitidos: 1 });
    expect(resultado.errores).toHaveLength(1);
    expect(productos.validarEmpresaCatalogo).toHaveBeenCalledWith(EMPRESA);
  });

  it('rechaza importaciones vacías o demasiado grandes', async () => {
    const { servicio } = crear();
    await expect(servicio.importarProductos(EMPRESA, [])).rejects.toMatchObject({
      codigo: 'IMPORTACION_VACIA',
    });
    await expect(
      servicio.importarProductos(
        EMPRESA,
        Array.from({ length: 201 }, () => ({
          sku: 'A-1',
          nombre: 'A',
          precio_base: '1.00',
          stock: 1,
        })),
      ),
    ).rejects.toMatchObject({ codigo: 'IMPORTACION_DEMASIADO_GRANDE' });
  });

  it('escanea productos, servicios y códigos inexistentes', async () => {
    const { servicio, productos, servicios } = crear();
    productos.encontrarPorCodigoBarras.mockResolvedValueOnce({ id: 'p1' });
    await expect(servicio.escanear(EMPRESA, '750')).resolves.toMatchObject({ tipo: 'producto' });

    servicios.encontrarPorCodigo.mockResolvedValueOnce({ id: 's1' });
    await expect(servicio.escanear(EMPRESA, 'SERV-1')).resolves.toMatchObject({ tipo: 'servicio' });

    await expect(servicio.escanear(EMPRESA, '')).rejects.toMatchObject({
      codigo: 'CODIGO_REQUERIDO',
    });
    await expect(servicio.escanear(EMPRESA, 'nada')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('lista y crea combos con costo actual de componentes', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ n: 1 });
    (pg.query as Mock).mockResolvedValueOnce([COMBO_ROW]).mockResolvedValueOnce([PRODUCTO]);

    const pagina = await servicio.listarCombos(EMPRESA, { q: 'combo', estado: 'activo' });
    expect(pagina.items[0]).toMatchObject({ costo_total: '20.00', margen: 33.3 });

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(PRODUCTO)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(COMBO_ROW);
    (pg.query as Mock).mockResolvedValueOnce([PRODUCTO]);
    const creado = await servicio.crearCombo(EMPRESA, {
      sku: 'combo-01',
      nombre: 'Combo',
      precio: '30.00',
      componentes: [{ sku: PRODUCTO.sku, cantidad: 2 }],
    });
    expect(creado).toMatchObject({ sku: 'COMBO-01', componentes: [expect.objectContaining({ cantidad: 2 })] });
  });

  it('valida combos sin componentes, duplicados y códigos repetidos', async () => {
    const { servicio, pg } = crear();
    await expect(
      servicio.crearCombo(EMPRESA, {
        sku: 'COMBO-1',
        nombre: 'Combo',
        precio: '10.00',
        componentes: [],
      }),
    ).rejects.toMatchObject({ codigo: 'COMBO_SIN_COMPONENTES' });

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(PRODUCTO)
      .mockResolvedValueOnce(PRODUCTO);
    await expect(
      servicio.crearCombo(EMPRESA, {
        sku: 'COMBO-1',
        nombre: 'Combo',
        precio: '10.00',
        componentes: [
          { sku: 'SKU-01', cantidad: 1 },
          { sku: 'SKU-01', cantidad: 1 },
        ],
      }),
    ).rejects.toMatchObject({ codigo: 'COMPONENTE_DUPLICADO' });

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(PRODUCTO)
      .mockResolvedValueOnce({ id: 'existe' });
    await expect(
      servicio.crearCombo(EMPRESA, {
        sku: 'COMBO-1',
        nombre: 'Combo',
        precio: '10.00',
        componentes: [{ sku: 'SKU-01', cantidad: 1 }],
      }),
    ).rejects.toBeInstanceOf(ConflictError);
  });

  it('obtiene, actualiza y elimina combos', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(COMBO_ROW)
      .mockResolvedValueOnce(COMBO_ROW)
      .mockResolvedValueOnce(PRODUCTO)
      .mockResolvedValueOnce(COMBO_ROW)
      .mockResolvedValueOnce({ id: COMBO_ROW.id });
    (pg.query as Mock).mockResolvedValue([PRODUCTO]);

    await expect(servicio.obtenerCombo(COMBO_ROW.id, EMPRESA)).resolves.toMatchObject({
      id: COMBO_ROW.id,
    });
    await expect(
      servicio.actualizarCombo(COMBO_ROW.id, EMPRESA, { nombre: 'Nuevo' }),
    ).resolves.toBeDefined();
    await expect(servicio.eliminarCombo(COMBO_ROW.id, EMPRESA)).resolves.toBeUndefined();
  });

  it('crea promociones 2x1 y valida tipos', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(PRODUCTO).mockResolvedValueOnce(PROMO_ROW);
    const promo = await servicio.crearPromocion(EMPRESA, {
      nombre: '2x1',
      tipo: '2x1',
      sku: PRODUCTO.sku,
      fecha_desde: '2026-10-01',
      fecha_hasta: '2026-12-31',
    });
    expect(promo).toMatchObject({ tipo: '2x1', cantidad_requerida: 2, cantidad_gratis: 1 });

    const sinCantidades = crear().servicio;
    await expect(
      sinCantidades.crearPromocion(EMPRESA, {
        nombre: 'Regalo',
        tipo: 'regalo',
        fecha_desde: '2026-10-01',
        fecha_hasta: '2026-12-31',
      }),
    ).rejects.toMatchObject({ codigo: 'PROMOCION_INVALIDA' });
    const descuentoInvalido = crear().servicio;
    await expect(
      descuentoInvalido.crearPromocion(EMPRESA, {
        nombre: 'Descuento',
        tipo: 'descuento_porcentaje',
        descuento_porcentaje: 200,
        fecha_desde: '2026-10-01',
        fecha_hasta: '2026-12-31',
      }),
    ).rejects.toMatchObject({ codigo: 'PROMOCION_INVALIDA' });
  });

  it('lista, obtiene, actualiza y elimina promociones', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ n: 1 });
    (pg.query as Mock).mockResolvedValueOnce([PROMO_ROW]);
    const pagina = await servicio.listarPromociones(EMPRESA, { estado: 'activo' });
    expect(pagina.items[0]).toMatchObject({ id: PROMO_ROW.id });

    (pg.queryOne as Mock).mockResolvedValueOnce(PROMO_ROW);
    await expect(servicio.obtenerPromocion(PROMO_ROW.id, EMPRESA)).resolves.toMatchObject({
      id: PROMO_ROW.id,
    });

    (pg.queryOne as Mock)
      .mockResolvedValueOnce(PROMO_ROW)
      .mockResolvedValueOnce(PRODUCTO)
      .mockResolvedValueOnce(PROMO_ROW);
    await expect(
      servicio.actualizarPromocion(PROMO_ROW.id, EMPRESA, { nombre: 'Nueva' }),
    ).resolves.toBeDefined();

    (pg.queryOne as Mock).mockResolvedValueOnce({ id: PROMO_ROW.id });
    await expect(servicio.eliminarPromocion(PROMO_ROW.id, EMPRESA)).resolves.toBeUndefined();
  });

  it('registra una entrada y publica stock actualizado', async () => {
    const { servicio, pg, client, outbox } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(PRODUCTO);
    const mov = await servicio.registrarMovimiento(
      EMPRESA,
      { tipo: 'entrada', motivo: 'compra', lineas: [{ sku: PRODUCTO.sku, cantidad: 5 }] },
      'u1',
    );
    expect(mov.id).toBe(MOV_ROW.id);
    expect(outbox.insertarEnTransaccion).toHaveBeenCalled();
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('UPDATE catalog.productos'),
      [PRODUCTO.id, EMPRESA, 15],
    );
  });

  it('rechaza salidas sin stock y traslados inválidos', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ ...PRODUCTO, stock: 1 });
    await expect(
      servicio.registrarMovimiento(EMPRESA, {
        tipo: 'salida',
        lineas: [{ sku: PRODUCTO.sku, cantidad: 5 }],
      }),
    ).rejects.toMatchObject({ codigo: 'STOCK_INSUFICIENTE' });

    await expect(
      servicio.registrarMovimiento(EMPRESA, {
        tipo: 'traslado',
        ubicacion_origen: 'A',
        ubicacion_destino: 'A',
        lineas: [{ sku: PRODUCTO.sku, cantidad: 1 }],
      }),
    ).rejects.toMatchObject({ codigo: 'TRASLADO_INVALIDO' });
  });

  it('registra un traslado sin cambiar el stock', async () => {
    const { servicio, pg, client, outbox } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(PRODUCTO);
    const mov = await servicio.registrarMovimiento(EMPRESA, {
      tipo: 'traslado',
      ubicacion_origen: 'A',
      ubicacion_destino: 'B',
      lineas: [{ sku: PRODUCTO.sku, cantidad: 2 }],
    });
    expect(mov.id).toBe(MOV_ROW.id);
    expect(outbox.insertarEnTransaccion).not.toHaveBeenCalled();
    expect(client.query).not.toHaveBeenCalledWith(
      expect.stringContaining('UPDATE catalog.productos'),
      expect.anything(),
    );
  });

  it('lista movimientos con filtro de tipo', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ n: 1 });
    (pg.query as Mock).mockResolvedValueOnce([MOV_ROW]);
    const pagina = await servicio.listarMovimientos(EMPRESA, { tipo: 'entrada' });
    expect(pagina.items[0]).toMatchObject({ id: MOV_ROW.id, tipo: 'entrada' });
  });

  it('valida movimientos vacíos o con demasiadas líneas', async () => {
    const { servicio } = crear();
    await expect(
      servicio.registrarMovimiento(EMPRESA, { tipo: 'entrada', lineas: [] }),
    ).rejects.toMatchObject({ codigo: 'LINEAS_REQUERIDAS' });
    await expect(
      servicio.registrarMovimiento(EMPRESA, {
        tipo: 'entrada',
        lineas: Array.from({ length: 201 }, () => ({ sku: 'SKU', cantidad: 1 })),
      }),
    ).rejects.toMatchObject({ codigo: 'DEMASIADAS_LINEAS' });
  });
});

describe('OperacionesService · ramas adicionales (R-PS-04)', () => {
  it('actualiza combo cambiando componentes y código de barras', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(COMBO_ROW)
      .mockResolvedValueOnce(PRODUCTO)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(COMBO_ROW);
    (pg.query as Mock).mockResolvedValue([PRODUCTO]);

    const combo = await servicio.actualizarCombo(COMBO_ROW.id, EMPRESA, {
      codigo_barras: '999',
      componentes: [{ sku: PRODUCTO.sku, cantidad: 3 }],
    });
    expect(combo.id).toBe(COMBO_ROW.id);
  });

  it('rechaza actualizar o eliminar combos inexistentes', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(
      servicio.actualizarCombo(COMBO_ROW.id, EMPRESA, { nombre: 'X' }),
    ).rejects.toBeInstanceOf(NotFoundError);
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.eliminarCombo(COMBO_ROW.id, EMPRESA)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('crea promociones de descuento y regalo válidas', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(PRODUCTO)
      .mockResolvedValueOnce(PROMO_ROW)
      .mockResolvedValueOnce(PRODUCTO)
      .mockResolvedValueOnce(PROMO_ROW)
      .mockResolvedValueOnce(PRODUCTO)
      .mockResolvedValueOnce(PROMO_ROW);

    await expect(
      servicio.crearPromocion(EMPRESA, {
        nombre: 'Descuento',
        tipo: 'descuento_cantidad',
        sku: PRODUCTO.sku,
        cantidad_requerida: 3,
        descuento_porcentaje: 10,
        fecha_desde: '2026-10-01',
        fecha_hasta: '2026-12-31',
      }),
    ).resolves.toBeDefined();
    await expect(
      servicio.crearPromocion(EMPRESA, {
        nombre: 'Regalo',
        tipo: 'regalo',
        sku: PRODUCTO.sku,
        cantidad_requerida: 2,
        cantidad_gratis: 1,
        fecha_desde: '2026-10-01',
        fecha_hasta: '2026-12-31',
      }),
    ).resolves.toBeDefined();
    await expect(
      servicio.crearPromocion(EMPRESA, {
        nombre: 'Porcentaje',
        tipo: 'descuento_porcentaje',
        sku: PRODUCTO.sku,
        descuento_porcentaje: 15,
        fecha_desde: '2026-10-01',
        fecha_hasta: '2026-12-31',
      }),
    ).resolves.toBeDefined();
  });

  it('rechaza vigencias invertidas', async () => {
    const { servicio } = crear();
    await expect(
      servicio.crearPromocion(EMPRESA, {
        nombre: 'Promo',
        tipo: '2x1',
        fecha_desde: '2026-12-31',
        fecha_hasta: '2026-10-01',
      }),
    ).rejects.toMatchObject({ codigo: 'VIGENCIA_INVALIDA' });
  });

  it('permite ajuste absoluto y salida con stock negativo configurado', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({
        empresa_id: EMPRESA,
        metodo_costeo: 'promedio',
        permitir_stock_negativo: true,
        alerta_stock_bajo: true,
        actualizar_costo_en_compra: true,
        unidad_defecto: 'Unidad (u)',
        actualizado_en: '2026-10-30T10:00:00.000Z',
      })
      .mockResolvedValueOnce({ ...PRODUCTO, stock: 1 });
    await expect(
      servicio.registrarMovimiento(EMPRESA, {
        tipo: 'salida',
        lineas: [{ sku: PRODUCTO.sku, cantidad: 5 }],
      }),
    ).resolves.toBeDefined();

    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(PRODUCTO);
    await expect(
      servicio.registrarMovimiento(EMPRESA, {
        tipo: 'ajuste',
        lineas: [{ sku: PRODUCTO.sku, cantidad: 3 }],
      }),
    ).resolves.toBeDefined();
  });
});
