import { aProductoPublico } from './productos.publico';
import type { Producto } from './productos.service';

const crearProducto = (overrides: Partial<Producto> = {}): Producto => ({
  id: 'p-1',
  empresa_id: 'e-1',
  sku: 'SKU-1',
  nombre: 'Leche 1L',
  descripcion: 'Leche entera',
  categoria: 'lacteos',
  precio_base: '50.00',
  stock: 3,
  estado: 'disponible',
  estado_operativo: 'activo',
  costo: '30.00',
  marca: 'Marca',
  codigo_barras: '7501234567890',
  unidad: 'Unidad (u)',
  permite_fracciones: false,
  unidad_compra: 'Caja',
  contenido_cantidad: 1,
  contenido_unidad: 'L',
  tipo_precio: 'fijo',
  tipo_costo: 'fijo',
  stock_minimo: 2,
  stock_maximo: 10,
  garantia_meses: null,
  es_materia_prima: false,
  ubicacion: 'A-1',
  actualizado_en: '2026-01-02T00:00:00.000Z',
  creado_en: '2026-01-01T00:00:00.000Z',
  producto_padre_id: null,
  atributos: {},
  tiene_variantes: false,
  imagenes: [
    { id: 'i-1', objeto_id: 'o-1', url: '/api/v1/storage/public/objetos/o-1', orden: 0, es_principal: true },
  ],
  precio_desde: '50.00',
  stock_total: 3,
  ...overrides,
});

describe('proyección pública del catálogo (R-DS-3 / R-C-3)', () => {
  it('debe omitir costo, stock exacto, umbrales y datos internos', () => {
    const producto = crearProducto();
    const publico = aProductoPublico(producto);

    expect(publico).toMatchObject({
      id: 'p-1',
      empresa_id: 'e-1',
      nombre: 'Leche 1L',
      precio_base: '50.00',
      disponible: true,
    });

    for (const campo of [
      'costo',
      'tipo_costo',
      'stock',
      'stock_total',
      'stock_minimo',
      'stock_maximo',
      'codigo_barras',
      'ubicacion',
      'es_materia_prima',
      'estado_operativo',
    ]) {
      expect(publico).not.toHaveProperty(campo);
    }
  });

  it('debe derivar disponible del estado y no del stock exacto', () => {
    expect(aProductoPublico(crearProducto({ estado: 'agotado', stock: 0 })).disponible).toBe(false);
    expect(aProductoPublico(crearProducto({ estado: 'disponible', stock: 99 })).disponible).toBe(true);
  });

  it('debe aplicar la misma lista blanca a las variantes', () => {
    const variante = crearProducto({
      id: 'v-1',
      sku: 'SKU-1-ROJO',
      producto_padre_id: 'p-1',
      stock: 1,
      costo: '25.00',
      stock_minimo: 1,
    });
    const publico = aProductoPublico(crearProducto({ variantes: [variante] }));

    expect(publico.variantes).toHaveLength(1);
    expect(publico.variantes?.[0]).toMatchObject({ id: 'v-1', disponible: true });
    expect(publico.variantes?.[0]).not.toHaveProperty('stock');
    expect(publico.variantes?.[0]).not.toHaveProperty('costo');
    expect(publico.variantes?.[0]).not.toHaveProperty('stock_minimo');
  });
});
