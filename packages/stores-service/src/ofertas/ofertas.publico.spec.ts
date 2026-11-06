import type { Tienda } from '../tiendas/tiendas.service';
import type { Oferta } from './ofertas.service';
import { aOfertaPublica, aTiendaPublica } from './ofertas.publico';

const tienda = {
  id: 't-1',
  empresa_id: 'e-1',
  propietario_usuario_id: 'u-p',
  participacion_porcentaje: 100,
  vendedor_id: 'u-v',
  nombre: 'Mi tienda',
  slug: 'mi-tienda',
  descripcion: 'Tienda de prueba',
  logo_objeto_id: null,
  background_objeto_id: null,
  background_media_type: null,
  background_poster_objeto_id: null,
  background_alt: '',
  background_overlay: 35,
  background_position: 'center',
  background_activo: false,
  origen_personalizacion: { plantilla: 'plataforma', tema: 'plataforma' },
  creado_en: '2026-01-01T00:00:00.000Z',
  actualizado_en: '2026-01-02T00:00:00.000Z',
} as unknown as Tienda;

const oferta: Oferta = {
  id: 'of-1',
  tienda_id: 't-1',
  vendedor_id: 'u-v',
  empresa_id: 'e-1',
  participacion_porcentaje: 100,
  producto_id: 'p-1',
  producto_padre_id: null,
  sku: 'SKU-1',
  producto_nombre: 'Leche 1L',
  atributos: {},
  imagen_objeto_id: null,
  imagen_url: null,
  margen: 10,
  precio_base: '50.00',
  precio_venta: '55.00',
  stock: 1,
  estado: 'activa',
  creado_en: '2026-01-01T00:00:00.000Z',
};

describe('proyección pública del storefront (R-DS-3 / R-C-3)', () => {
  it('debe ocultar los identificadores internos y el reparto de la tienda', () => {
    const publica = aTiendaPublica(tienda);

    expect(publica).toMatchObject({ id: 't-1', empresa_id: 'e-1', nombre: 'Mi tienda' });
    for (const campo of [
      'propietario_usuario_id',
      'participacion_porcentaje',
      'vendedor_id',
      'creado_en',
      'actualizado_en',
      'origen_personalizacion',
    ]) {
      expect(publica).not.toHaveProperty(campo);
    }
  });

  it('debe ocultar margen, precio base, stock exacto e IDs internos de la oferta', () => {
    const publica = aOfertaPublica(oferta);

    expect(publica).toMatchObject({
      id: 'of-1',
      producto_id: 'p-1',
      precio_venta: '55.00',
      disponible: true,
    });
    for (const campo of [
      'tienda_id',
      'vendedor_id',
      'empresa_id',
      'participacion_porcentaje',
      'margen',
      'precio_base',
      'stock',
    ]) {
      expect(publica).not.toHaveProperty(campo);
    }
  });

  it('debe derivar disponible del estado y el stock, sin exponer el stock', () => {
    expect(aOfertaPublica({ ...oferta, stock: 0 }).disponible).toBe(false);
    expect(aOfertaPublica({ ...oferta, estado: 'inactiva' }).disponible).toBe(false);
    expect(aOfertaPublica({ ...oferta, stock: 5 }).disponible).toBe(true);
  });
});
