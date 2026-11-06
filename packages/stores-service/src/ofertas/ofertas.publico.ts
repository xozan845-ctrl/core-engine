import type { Tienda } from '../tiendas/tiendas.service';
import type { Oferta } from './ofertas.service';

/**
 * Proyección pública del storefront (R-DS-3 / R-C-3).
 *
 * La tienda conserva los campos de presentación (nombre, slug, tema, hero,
 * contenido), pero pierde los identificadores internos de propietario/vendedor
 * y el reparto comercial. La oferta usa una lista blanca: no expone margen,
 * precio base, stock exacto ni IDs internos; solo el precio de venta y la
 * disponibilidad.
 */

export type TiendaPublica = Omit<
  Tienda,
  | 'propietario_usuario_id'
  | 'participacion_porcentaje'
  | 'vendedor_id'
  | 'creado_en'
  | 'actualizado_en'
  | 'origen_personalizacion'
>;

export function aTiendaPublica(tienda: Tienda): TiendaPublica {
  const {
    propietario_usuario_id: _propietario,
    participacion_porcentaje: _participacion,
    vendedor_id: _vendedor,
    creado_en: _creado,
    actualizado_en: _actualizado,
    origen_personalizacion: _origen,
    ...publica
  } = tienda;
  return publica;
}

export interface OfertaPublica {
  id: string;
  producto_id: string;
  producto_padre_id: string | null;
  sku: string;
  producto_nombre: string;
  atributos: Record<string, unknown>;
  imagen_objeto_id: string | null;
  imagen_url: string | null;
  precio_venta: string;
  estado: string;
  disponible: boolean;
  creado_en: string;
}

export function aOfertaPublica(oferta: Oferta): OfertaPublica {
  return {
    id: oferta.id,
    producto_id: oferta.producto_id,
    producto_padre_id: oferta.producto_padre_id,
    sku: oferta.sku,
    producto_nombre: oferta.producto_nombre,
    atributos: oferta.atributos,
    imagen_objeto_id: oferta.imagen_objeto_id,
    imagen_url: oferta.imagen_url,
    precio_venta: oferta.precio_venta,
    estado: oferta.estado,
    disponible: oferta.estado === 'activa' && oferta.stock > 0,
    creado_en: oferta.creado_en,
  };
}
