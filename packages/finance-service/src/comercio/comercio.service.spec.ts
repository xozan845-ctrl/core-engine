import { DomainError, NotFoundError, ROLES } from '@core/shared';
import { ComercioService } from './comercio.service';

const CREADO = new Date('2026-01-01T10:00:00.000Z');
const ACTUALIZADO = new Date('2026-01-02T10:00:00.000Z');

function fila(datos: Record<string, unknown>, id = 'r1') {
  return { id, datos, creado_en: CREADO, actualizado_en: ACTUALIZADO };
}

function crear() {
  const client = { query: vi.fn() };
  const pg = {
    query: vi.fn().mockResolvedValue([]),
    queryOne: vi.fn(),
    transaccion: vi.fn(async (fn: (c: unknown) => Promise<unknown>) => fn(client)),
  };
  const servicio = new ComercioService(pg as never);
  return { servicio, pg, client };
}

const empresa = {
  user_id: 'u1',
  email: 'empresa@test.dev',
  rol: ROLES.EMPRESARIO,
  tenant_id: 't1',
};
const vendedor = { user_id: 'v1', email: 'vendedor@test.dev', rol: ROLES.VENDEDOR };
const admin = { user_id: 'a1', email: 'admin@test.dev', rol: ROLES.ADMIN };
const cliente = { user_id: 'c1', email: 'comprador@test.dev', rol: ROLES.COMPRADOR };

describe('ComercioService', () => {
  describe('ámbito y consultas', () => {
    it('lista registros de empresa y normaliza fechas', async () => {
      const { servicio, pg } = crear();
      pg.query.mockResolvedValue([fila({ bank: 'BAC', balanceCents: 100 })]);

      const resultado = await servicio.listarBancos(empresa);

      expect(resultado[0]).toMatchObject({
        id: 'r1',
        bank: 'BAC',
        balanceCents: 100,
        createdAt: CREADO.toISOString(),
        updatedAt: ACTUALIZADO.toISOString(),
      });
      expect(pg.query.mock.calls[0][1]).toEqual(['bancos-cuentas', 't1', null]);
    });

    it('aísla al vendedor por propietario', async () => {
      const { servicio, pg } = crear();
      await servicio.listarCreditos(vendedor);
      expect(pg.query.mock.calls[0][1]).toEqual(['creditos', null, 'v1']);
    });

    it('el admin puede acotar por empresa', async () => {
      const { servicio, pg } = crear();
      await servicio.listar('gastos', admin, 't9');
      expect(pg.query.mock.calls[0][1]).toEqual(['gastos', 't9', null]);
    });

    it('rechaza roles ajenos al comercio', async () => {
      const { servicio } = crear();
      await expect(servicio.listarClientes(cliente)).rejects.toBeInstanceOf(DomainError);
    });

    it('rechaza recursos de empresa para un vendedor', async () => {
      const { servicio } = crear();
      await expect(servicio.listarGastos(vendedor)).rejects.toMatchObject({
        codigo: 'ACCESO_DENEGADO',
      });
    });

    it('exige tenant al empresario', async () => {
      const { servicio } = crear();
      await expect(
        servicio.listarClientes({ ...empresa, tenant_id: undefined }),
      ).rejects.toMatchObject({ codigo: 'TENANT_REQUERIDO' });
    });

    it('obtiene un registro y falla si no existe', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValueOnce(fila({ name: 'Cliente' }));
      await expect(servicio.obtener('clientes', 'r1', empresa)).resolves.toMatchObject({
        name: 'Cliente',
      });
      pg.queryOne.mockResolvedValueOnce(null);
      await expect(servicio.obtener('clientes', 'r2', empresa)).rejects.toBeInstanceOf(
        NotFoundError,
      );
    });

    it('crea un registro y exige ámbito al admin', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ name: 'Cliente' }));
      await expect(servicio.crearCliente({ name: 'Cliente' }, empresa)).resolves.toMatchObject({
        name: 'Cliente',
      });
      await expect(servicio.crearCliente({ name: 'Cliente' }, admin)).rejects.toMatchObject({
        codigo: 'TENANT_REQUERIDO',
      });
    });

    it('actualiza y elimina con ámbito', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValueOnce(fila({ name: 'Nuevo' }));
      await expect(
        servicio.actualizarCliente('r1', { name: 'Nuevo' }, empresa),
      ).resolves.toMatchObject({ name: 'Nuevo' });

      pg.queryOne.mockResolvedValueOnce(null);
      await expect(
        servicio.actualizarCliente('r2', { name: 'Nuevo' }, empresa),
      ).rejects.toBeInstanceOf(NotFoundError);

      pg.queryOne.mockResolvedValueOnce({ id: 'r1' });
      await expect(servicio.eliminar('clientes', 'r1', empresa)).resolves.toBeUndefined();

      pg.queryOne.mockResolvedValueOnce(null);
      await expect(servicio.eliminar('clientes', 'r2', empresa)).rejects.toBeInstanceOf(
        NotFoundError,
      );
    });
  });

  describe('bancos, créditos, gastos y cuentas por pagar', () => {
    it('registra una cuenta bancaria', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ bank: 'BAC', balanceCents: 500 }));
      const resultado = await servicio.crearBanco(
        {
          bank: 'BAC',
          alias: 'Operativa',
          accountNumber: '123',
          type: 'ahorro',
          balanceCents: 500,
        },
        empresa,
      );
      expect(resultado).toMatchObject({ bank: 'BAC' });
      const datos = JSON.parse(pg.queryOne.mock.calls[0][1][3] as string);
      expect(datos).toMatchObject({ bank: 'BAC', balanceCents: 500 });
      expect(datos.createdAt).toBeTruthy();
    });

    it('actualiza una cuenta bancaria', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ bank: 'LAFISE' }));
      await expect(
        servicio.actualizarBanco('r1', { alias: 'Nueva' }, empresa),
      ).resolves.toMatchObject({ bank: 'LAFISE' });
    });

    it('registra y paga un crédito', async () => {
      const { servicio, pg } = crear();
      pg.queryOne
        .mockResolvedValueOnce(fila({ customer: 'Ana', balanceCents: 100 }))
        .mockResolvedValueOnce(fila({ customer: 'Ana', balanceCents: 0 }));
      const creado = await servicio.crearCredito(
        { customer: 'Ana', concept: 'Venta', dueDate: '2026-02-01', amountCents: 100 },
        empresa,
      );
      expect(creado).toMatchObject({ customer: 'Ana' });
      const datos = JSON.parse(pg.queryOne.mock.calls[0][1][3] as string);
      expect(datos).toMatchObject({ date: expect.any(String), balanceCents: 100 });

      await servicio.pagarCredito('r1', empresa);
      expect(pg.queryOne.mock.calls[1][1][3]).toContain('"balanceCents":0');
    });

    it('registra, actualiza y paga un gasto', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ concept: 'Renta' }));
      await servicio.crearGasto(
        {
          date: '2026-01-01',
          concept: 'Renta',
          category: 'servicios',
          type: 'programado',
          amountCents: 1000,
          paymentMethod: 'transferencia',
        },
        empresa,
      );
      const datos = JSON.parse(pg.queryOne.mock.calls[0][1][3] as string);
      expect(datos).toMatchObject({ status: 'pendiente', includesVat: false, irRetentionCents: 0 });

      await servicio.actualizarGasto('r1', { amountCents: 2000 }, empresa);
      await servicio.actualizar('gastos', 'r1', { status: 'pagado' }, empresa);
      expect(pg.queryOne).toHaveBeenCalledTimes(3);
    });

    it('registra y paga una cuenta por pagar', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ supplier: 'Proveedor' }));
      await servicio.crearCuentaPorPagar(
        { supplier: 'Proveedor', concept: 'Mercancía', dueDate: '2026-02-01', amountCents: 500 },
        empresa,
      );
      const datos = JSON.parse(pg.queryOne.mock.calls[0][1][3] as string);
      expect(datos).toMatchObject({ status: 'pendiente', paidAt: '' });

      await servicio.pagarCuentaPorPagar('r1', empresa);
      expect(pg.queryOne.mock.calls[1][1][3]).toContain('"status":"pagada"');
    });

    it('registra un proveedor', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ name: 'Distribuidora' }));
      await servicio.crearProveedor({ name: 'Distribuidora' }, empresa);
      const datos = JSON.parse(pg.queryOne.mock.calls[0][1][3] as string);
      expect(datos).toMatchObject({ name: 'Distribuidora', ruc: '', phone: '' });
    });
  });

  describe('compras y órdenes', () => {
    const lineas = [
      {
        productId: 'p1',
        name: 'Caja',
        sku: 'SKU-1',
        quantity: 2,
        unitCostCents: 500,
        salePriceCents: 800,
      },
    ];

    it('registra una compra con totales y número generado', async () => {
      const { servicio, pg } = crear();
      pg.query.mockResolvedValue([]);
      pg.queryOne.mockResolvedValue(fila({ number: 'C-000001' }));
      await servicio.crearCompra(
        {
          date: '2026-01-01',
          paymentMethod: 'efectivo',
          status: 'recibida',
          origin: 'productos',
          costType: 'inventario',
          lines: lineas,
        },
        empresa,
      );
      const datos = JSON.parse(pg.queryOne.mock.calls[0][1][3] as string);
      expect(datos).toMatchObject({
        number: 'C-000001',
        subtotalCents: 1000,
        totalCents: 1000,
        retentionCents: 0,
      });
      expect(datos.lines[0]).toMatchObject({ name: 'Caja', unitCostCents: 500 });
      expect(datos.lines[0].id).toBeTruthy();
    });

    it('registra compras directas y de producto nuevo', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ number: 'C-000002' }));
      await servicio.crearCompraDirecta(
        {
          date: '2026-01-01',
          concept: 'Flete',
          costType: 'gasto',
          amountCents: 300,
          paymentMethod: 'efectivo',
        },
        empresa,
      );
      expect(JSON.parse(pg.queryOne.mock.calls[0][1][3] as string)).toMatchObject({
        origin: 'directa',
        subtotalCents: 300,
      });

      await servicio.crearCompraProductoNuevo(
        {
          date: '2026-01-01',
          productName: 'Nuevo',
          paymentMethod: 'credito',
          unitCostCents: 100,
          salePriceCents: 200,
          quantity: 3,
        },
        empresa,
      );
      expect(JSON.parse(pg.queryOne.mock.calls[1][1][3] as string)).toMatchObject({
        origin: 'productos',
        subtotalCents: 300,
      });
    });

    it('anula compras', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ status: 'anulada' }));
      await expect(servicio.anularCompra('r1', empresa)).resolves.toMatchObject({
        status: 'anulada',
      });
    });

    it('crea una orden y la convierte en compra', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValueOnce(fila({ number: 'OC-000001', status: 'pendiente' }));
      await servicio.crearOrdenCompra(
        { date: '2026-01-01', supplierName: 'Prov', lines: lineas },
        empresa,
      );
      expect(JSON.parse(pg.queryOne.mock.calls[0][1][3] as string)).toMatchObject({
        status: 'pendiente',
        totalCents: 1000,
      });

      const orden = fila({
        number: 'OC-000001',
        status: 'pendiente',
        supplierId: null,
        supplierName: 'Prov',
        lines: [{ id: 'l1', name: 'Caja', quantity: 1, unitCostCents: 100 }],
      });
      pg.queryOne
        .mockResolvedValueOnce(orden)
        .mockResolvedValueOnce(fila({ number: 'C-000001' }))
        .mockResolvedValueOnce(fila({ status: 'convertida' }));
      pg.query.mockResolvedValue([]);
      await expect(servicio.convertirOrdenCompra('o1', empresa)).resolves.toMatchObject({
        status: 'convertida',
      });
    });

    it('rechaza convertir una orden no pendiente y anula órdenes', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValueOnce(fila({ status: 'anulada' }));
      await expect(servicio.convertirOrdenCompra('o1', empresa)).rejects.toMatchObject({
        codigo: 'ORDEN_NO_PENDIENTE',
      });
      pg.queryOne.mockResolvedValueOnce(fila({ status: 'anulada' }));
      await expect(servicio.anularOrdenCompra('o1', empresa)).resolves.toMatchObject({
        status: 'anulada',
      });
    });

    it('inicializa y actualiza la configuración de documentos', async () => {
      const { servicio, pg } = crear();
      pg.query.mockResolvedValueOnce([]).mockResolvedValueOnce([fila({ logoSize: 'mediano' })]);
      pg.queryOne
        .mockResolvedValueOnce(fila({ logoSize: 'mediano' }))
        .mockResolvedValueOnce(fila({ logoSize: 'grande' }));
      const inicial = await servicio.obtenerConfiguracionDocumentos(empresa);
      expect(inicial).toMatchObject({ logoSize: 'mediano' });

      await servicio.actualizarConfiguracionDocumentos(
        {
          logoSize: 'grande',
          headerColor: '#000',
          matchTableColor: true,
          footerMessage: 'Gracias',
          showRuc: true,
          showAddress: false,
          showPhone: false,
        },
        empresa,
      );
      expect(pg.queryOne).toHaveBeenCalledTimes(2);
    });
  });

  describe('caja', () => {
    it('gestiona métodos de pago y sus guardas', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ name: 'Efectivo' }));
      await servicio.crearMetodoPago({ name: 'Efectivo', kind: 'efectivo' }, empresa);
      expect(JSON.parse(pg.queryOne.mock.calls[0][1][3] as string)).toMatchObject({
        name: 'Efectivo',
        icon: 'payments',
        active: true,
      });

      pg.queryOne.mockResolvedValueOnce(fila({ name: 'Tarjeta' }));
      await servicio.actualizarMetodoPago('r1', { active: false }, empresa);

      pg.query.mockResolvedValueOnce([fila({ id: 'm1' })]);
      await expect(servicio.eliminarMetodoPago('m1', empresa)).rejects.toMatchObject({
        codigo: 'ULTIMO_METODO',
      });

      pg.query
        .mockResolvedValueOnce([fila({ id: 'm1' }), fila({ id: 'm2' })])
        .mockResolvedValueOnce([fila({ paymentMethod: 'm1' })]);
      await expect(servicio.eliminarMetodoPago('m1', empresa)).rejects.toMatchObject({
        codigo: 'METODO_EN_USO',
      });

      pg.query
        .mockResolvedValueOnce([fila({ id: 'm1' }), fila({ id: 'm2' })])
        .mockResolvedValueOnce([]);
      pg.queryOne.mockResolvedValueOnce({ id: 'm1' });
      await expect(servicio.eliminarMetodoPago('m1', empresa)).resolves.toBeUndefined();
    });

    it('registra movimientos con defaults y los anula', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ type: 'ingreso', status: 'completado' }));
      await servicio.crearMovimientoCaja(
        { date: '2026-01-01', type: 'ingreso', concept: 'Venta', amountCents: 1000, paymentMethod: 'efectivo' },
        empresa,
      );
      expect(JSON.parse(pg.queryOne.mock.calls[0][1][3] as string)).toMatchObject({
        currency: 'NIO',
        originalAmountCents: 1000,
        exchangeRate: 1,
        category: 'otro',
        source: 'manual',
        status: 'completado',
      });
      await servicio.anularMovimientoCaja('r1', empresa);
      expect(pg.queryOne.mock.calls[1][1][3]).toContain('"status":"anulado"');
    });

    it('calcula el preview del cierre y bloquea duplicados', async () => {
      const { servicio, pg } = crear();
      const movimientos = [
        fila({ date: '2026-01-01', type: 'ingreso', amountCents: 1000, status: 'completado' }),
        fila({ date: '2026-01-01', type: 'egreso', amountCents: 200, status: 'completado' }),
        fila({ date: '2026-01-02', type: 'ingreso', amountCents: 500, status: 'completado' }),
        fila({ date: '2026-01-02', type: 'ingreso', amountCents: 900, status: 'anulado' }),
      ];
      pg.query.mockResolvedValueOnce(movimientos).mockResolvedValueOnce([]);
      const preview = await servicio.previewCierre(empresa, '2026-01-02');
      expect(preview).toMatchObject({
        openingBalanceCents: 800,
        incomeCents: 500,
        expenseCents: 0,
        expectedBalanceCents: 1300,
        movementsCount: 1,
        existing: null,
      });

      pg.query
        .mockResolvedValueOnce(movimientos)
        .mockResolvedValueOnce([fila({ date: '2026-01-02' })]);
      await expect(
        servicio.crearCierre({ date: '2026-01-02', countedBalanceCents: 1300 }, empresa),
      ).rejects.toMatchObject({ codigo: 'CIERRE_EXISTENTE' });

      pg.query.mockResolvedValueOnce(movimientos).mockResolvedValueOnce([]);
      pg.queryOne.mockResolvedValueOnce(fila({ date: '2026-01-02' }));
      await servicio.crearCierre({ date: '2026-01-02', countedBalanceCents: 1200 }, empresa);
      expect(JSON.parse(pg.queryOne.mock.calls[0][1][3] as string)).toMatchObject({
        differenceCents: -100,
        movementsCount: 1,
      });
    });

    it('gestiona el fondo y los movimientos de caja chica', async () => {
      const { servicio, pg } = crear();
      pg.query.mockResolvedValueOnce([]);
      pg.queryOne.mockResolvedValueOnce(fila({ amountCents: 0, responsible: '' }));
      await servicio.obtenerFondoCajaChica(empresa);
      expect(JSON.parse(pg.queryOne.mock.calls[0][1][3] as string)).toMatchObject({
        amountCents: 0,
      });

      pg.query.mockResolvedValueOnce([fila({ amountCents: 100 })]);
      pg.queryOne.mockResolvedValueOnce(fila({ amountCents: 300 }));
      await servicio.guardarFondoCajaChica({ amountCents: 300, responsible: 'Ana' }, empresa);

      pg.queryOne.mockResolvedValueOnce(fila({ amountCents: 300 }));
      await servicio.crearMovimientoCajaChica(
        { date: '2026-01-01', type: 'gasto', concept: 'Taxi', amountCents: 50 },
        empresa,
      );
      expect(pg.queryOne).toHaveBeenCalledTimes(3);
    });

    it('registra una transferencia entre bancos y caja', async () => {
      const { servicio, client } = crear();
      client.query.mockResolvedValue({ rowCount: 1, rows: [fila({ status: 'completado' })] });

      const resultado = await servicio.crearTransferencia(
        {
          date: '2026-01-01',
          fromId: 'b1',
          fromLabel: 'BAC',
          fromKind: 'bank',
          toId: 'cash',
          toLabel: 'Caja',
          toKind: 'cash',
          amountCents: 200,
          concept: 'Retiro',
        },
        empresa,
      );

      expect(resultado).toMatchObject({ status: 'completado' });
      expect(client.query).toHaveBeenCalledWith(
        expect.stringContaining("jsonb_set(datos, '{balanceCents}'"),
        expect.arrayContaining(['b1', 't1', 200, null]),
      );
      expect(client.query).toHaveBeenCalledTimes(4);
    });

    it('rechaza transferencias al mismo destino y saldos insuficientes', async () => {
      const { servicio, client } = crear();
      await expect(
        servicio.crearTransferencia(
          {
            date: '2026-01-01',
            fromId: 'x',
            fromLabel: 'X',
            fromKind: 'cash',
            toId: 'x',
            toLabel: 'X',
            toKind: 'cash',
            amountCents: 1,
            concept: 'X',
          },
          empresa,
        ),
      ).rejects.toMatchObject({ codigo: 'TRANSFERENCIA_INVALIDA' });

      client.query
        .mockResolvedValueOnce({ rowCount: 0 })
        .mockResolvedValueOnce({ rowCount: 1 });
      await expect(
        servicio.crearTransferencia(
          {
            date: '2026-01-01',
            fromId: 'b1',
            fromLabel: 'BAC',
            fromKind: 'bank',
            toId: 'cash',
            toLabel: 'Caja',
            toKind: 'cash',
            amountCents: 200,
            concept: 'Retiro',
          },
          empresa,
        ),
      ).rejects.toMatchObject({ codigo: 'SALDO_INSUFICIENTE' });

      client.query
        .mockResolvedValueOnce({ rowCount: 0 })
        .mockResolvedValueOnce({ rowCount: 0 });
      await expect(
        servicio.crearTransferencia(
          {
            date: '2026-01-01',
            fromId: 'b1',
            fromLabel: 'BAC',
            fromKind: 'bank',
            toId: 'cash',
            toLabel: 'Caja',
            toKind: 'cash',
            amountCents: 200,
            concept: 'Retiro',
          },
          empresa,
        ),
      ).rejects.toBeInstanceOf(NotFoundError);
    });

    it('anula una transferencia y revierte bancos y movimientos', async () => {
      const { servicio, pg, client } = crear();
      pg.queryOne.mockResolvedValueOnce(
        fila({
          date: '2026-01-01',
          fromId: 'b1',
          fromKind: 'bank',
          toId: 'b2',
          toKind: 'bank',
          amountCents: 100,
          status: 'completado',
        }),
      );
      client.query.mockResolvedValue({ rowCount: 1, rows: [fila({ status: 'anulado' })] });

      const resultado = await servicio.anularTransferencia('tr1', empresa);

      expect(resultado).toMatchObject({ status: 'anulado' });
      expect(client.query).toHaveBeenCalledTimes(4);
    });

    it('no repite la anulación de una transferencia ya anulada', async () => {
      const { servicio, pg, client } = crear();
      pg.queryOne.mockResolvedValueOnce(fila({ status: 'anulado' }));
      await expect(servicio.anularTransferencia('tr1', empresa)).resolves.toMatchObject({
        status: 'anulado',
      });
      expect(client.query).not.toHaveBeenCalled();
    });
  });

  describe('clientes y códigos de barras', () => {
    it('crea y actualiza clientes', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValueOnce(fila({ name: 'Ana', source: 'manual' }));
      await servicio.crearCliente({ name: 'Ana' }, empresa);
      expect(JSON.parse(pg.queryOne.mock.calls[0][1][3] as string)).toMatchObject({
        name: 'Ana',
        source: 'manual',
        phone: '',
      });

      pg.queryOne.mockResolvedValueOnce(fila({ name: 'Ana María' }));
      await expect(
        servicio.actualizarCliente('r1', { name: 'Ana María' }, empresa),
      ).resolves.toMatchObject({ name: 'Ana María' });
    });

    it('importa clientes omitiendo vacíos y duplicados', async () => {
      const { servicio, pg } = crear();
      pg.query.mockResolvedValue([fila({ name: 'Ana', phone: '8888' })]);
      pg.queryOne.mockResolvedValue(fila({ name: 'Nuevo' }));

      const resultado = await servicio.importarClientes(
        {
          clientes: [
            { name: 'Ana', phone: '8888' },
            { name: '   ' },
            { name: 'Luis', phone: '7777' },
            { name: 'Luis', phone: '7777' },
          ],
        },
        empresa,
      );

      expect(resultado).toMatchObject({ imported: 1, skipped: 3 });
      expect(resultado.errors).toHaveLength(1);
    });

    it('rechaza importaciones mayores a 500 filas', async () => {
      const { servicio } = crear();
      await expect(
        servicio.importarClientes(
          { clientes: Array.from({ length: 501 }, () => ({ name: 'X' })) },
          empresa,
        ),
      ).rejects.toMatchObject({ codigo: 'IMPORTACION_DEMASIADO_GRANDE' });
    });

    it('genera códigos EAN13 y CODE128', async () => {
      const { servicio, pg } = crear();
      pg.queryOne.mockResolvedValue(fila({ code: '1234567890128' }));
      await servicio.crearCodigoBarras(
        { productName: 'Producto', sku: 'SKU', symbology: 'EAN13', copies: 2 },
        empresa,
      );
      const ean = JSON.parse(pg.queryOne.mock.calls[0][1][3] as string);
      expect(ean.code).toMatch(/^\d{13}$/);
      expect(ean.copies).toBe(2);

      pg.queryOne.mockResolvedValueOnce(fila({ code: 'C128-X' }));
      await servicio.crearCodigoBarras(
        { productName: 'Producto', sku: 'SKU', symbology: 'CODE128', copies: 1, code: 'CUSTOM' },
        empresa,
      );
      expect(JSON.parse(pg.queryOne.mock.calls[1][1][3] as string).code).toBe('CUSTOM');

      pg.queryOne.mockResolvedValueOnce(fila({ code: 'C128-Y' }));
      await servicio.crearCodigoBarras(
        { productName: 'Producto', sku: 'SKU', symbology: 'CODE128', copies: 1 },
        empresa,
      );
      expect(JSON.parse(pg.queryOne.mock.calls[2][1][3] as string).code).toMatch(/^C128-/);
    });
  });
});
