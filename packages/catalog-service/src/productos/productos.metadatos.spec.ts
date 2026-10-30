import type { Mock } from 'vitest';
import { ConflictError, DomainError } from '@core/shared';
import { ProductosService } from './productos.service';

const EMPRESA = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';

const fila = {
  id: '11111111-2222-3333-4444-555555555555',
  empresa_id: EMPRESA,
  sku: 'SKU-01',
  nombre: 'Teclado',
  descripcion: 'mecanico',
  categoria: 'perifericos',
  precio_base_cents: 150000,
  costo_cents: 100000,
  marca: 'Core',
  codigo_barras: '7501234567890',
  unidad: 'Unidad (u)',
  permite_fracciones: false,
  unidad_compra: 'Igual que la de venta',
  contenido_cantidad: 0,
  contenido_unidad: 'Sin contenido',
  tipo_precio: 'fijo',
  tipo_costo: 'fijo',
  stock_minimo: 2,
  stock_maximo: 20,
  garantia_meses: 12,
  es_materia_prima: false,
  ubicacion: 'A-1',
  estado_operativo: 'activo',
  actualizado_en: '2026-10-30T10:00:00.000Z',
  stock: 10,
  estado: 'disponible',
  creado_en: '2026-10-30T10:00:00.000Z',
};

function crear() {
  const client = { query: vi.fn().mockResolvedValue({ rows: [], rowCount: 1 }) };
  const pg = {
    query: vi.fn().mockResolvedValue([]),
    queryOne: vi.fn().mockResolvedValue(null),
    transaccion: vi.fn(async (fn: (c: unknown) => Promise<unknown>) => fn(client)),
  };
  const servicio = new ProductosService(pg as never);
  return { servicio, pg, client };
}

const datos = {
  empresa_id: EMPRESA,
  sku: 'sku-01',
  nombre: 'Teclado',
  precio_base: '1500.00',
  stock: 10,
  costo: '1000.00',
  marca: 'Core',
  codigo_barras: '7501234567890',
  unidad: 'Unidad (u)',
  stock_minimo: 2,
  stock_maximo: 20,
  garantia_meses: 12,
  es_materia_prima: true,
  ubicacion: 'A-1',
  estado_operativo: 'activo',
};

describe('ProductosService · metadatos y estado operativo (R-PS-01/03)', () => {
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

  afterEach(() => vi.unstubAllGlobals());

  it('persiste los metadatos y devuelve costo/estado operativo', async () => {
    const { servicio, pg, client } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(null);
    pg.queryOne.mockResolvedValueOnce(fila);

    const creado = await servicio.crear(datos);

    const insert = (client.query as Mock).mock.calls.find((c: unknown[]) =>
      String(c[0]).includes('INSERT INTO catalog.productos'),
    ) as [string, unknown[]];
    expect(insert[0]).toContain('costo_cents');
    expect(insert[1]).toContain(100000);
    expect(insert[1]).toContain('7501234567890');
    expect(creado).toMatchObject({
      costo: '1000.00',
      estado_operativo: 'activo',
      stock_minimo: 2,
      stock_maximo: 20,
    });
  });

  it('rechaza códigos de barras duplicados en la empresa', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce({ id: 'otro' });
    await expect(servicio.crear(datos)).rejects.toBeInstanceOf(ConflictError);
  });

  it('rechaza stock máximo menor al mínimo', async () => {
    const { servicio } = crear();
    await expect(servicio.crear({ ...datos, stock_minimo: 30 })).rejects.toMatchObject({
      codigo: 'METADATO_INVALIDO',
    });
  });

  it('rechaza metadata con enteros negativos', async () => {
    const { servicio } = crear();
    await expect(servicio.crear({ ...datos, contenido_cantidad: -1 })).rejects.toBeInstanceOf(
      DomainError,
    );
  });

  it('filtra inactivos por defecto y los incluye para el dueño', async () => {
    const { servicio, pg } = crear();
    await servicio.listar({ empresa_id: EMPRESA });
    expect(String((pg.query as Mock).mock.calls[0][0])).toContain(
      "estado_operativo = 'activo'",
    );

    await servicio.listar({ empresa_id: EMPRESA, incluir_inactivos: true });
    expect(String((pg.query as Mock).mock.calls[1][0])).not.toContain(
      "estado_operativo = 'activo'",
    );
  });

  it('actualiza el estado operativo y los metadatos', async () => {
    const { servicio, pg, client } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(fila)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ ...fila, estado_operativo: 'inactivo', ubicacion: 'B-2' });

    const actualizado = await servicio.actualizar(
      fila.id,
      { estado_operativo: 'inactivo', ubicacion: 'B-2' },
      EMPRESA,
    );

    const update = (client.query as Mock).mock.calls.find((c: unknown[]) =>
      String(c[0]).includes('UPDATE catalog.productos'),
    ) as [string, unknown[]];
    expect(update[0]).toContain('estado_operativo = $');
    expect(update[0]).toContain('actualizado_en = NOW()');
    expect(actualizado).toMatchObject({ estado_operativo: 'inactivo' });
  });
});
