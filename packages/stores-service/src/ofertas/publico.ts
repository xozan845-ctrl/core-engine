import type { Tienda } from '../tiendas/tiendas.service';
import type { Oferta } from './ofertas.service';

/**
 * Proyecciones públicas del storefront (R-DS-3 / R-C-3).
 *
 * La ruta GET /api/v2/tiendas/:id no lleva autenticación, así que solo puede
 * devolver listas blancas: nada de costo, margen, participación del vendedor,
 * stock interno de la empresa ni identificadores de usuario.
 */

export interface OfertaPublica {
  id: string;
  item_type: Oferta['item_type'];
  item_id: string;
  sku: string;
  producto_nombre: string;
  atributos: Record<string, unknown>;
  imagen_objeto_id: string | null;
  imagen_url: string | null;
  precio_venta: string;
  stock: number;
  estado: string;
  creado_en: string;
}

export interface TiendaPublica {
  id: string;
  nombre: string;
  slug: string;
  descripcion: string;
  logo_objeto_id: string | null;
  logo_url: string | null;
  background_objeto_id: string | null;
  background_media_type: Tienda['background_media_type'];
  background_poster_objeto_id: string | null;
  background_alt: string;
  background_overlay: number;
  background_position: Tienda['background_position'];
  background_activo: boolean;
  background_url: string | null;
  background_poster_url: string | null;
  background_movil_objeto_id: string | null;
  background_movil_media_type: Tienda['background_movil_media_type'];
  background_movil_poster_objeto_id: string | null;
  background_movil_alt: string;
  background_movil_activo: boolean;
  plantilla: Tienda['plantilla'];
  tema: Tienda['tema'];
  hero_titulo: string;
  hero_subtitulo: string;
  hero_cta_texto: string;
  hero_cta_url: string;
  seo_titulo: string;
  seo_descripcion: string;
  seo_imagen_objeto_id: string | null;
  seo_imagen_url: string | null;
  secciones: Tienda['secciones'];
  sobre_nosotros: string;
  politicas: Tienda['politicas'];
  redes: Tienda['redes'];
  anuncio_texto: string;
  anuncio_enlace: string;
  anuncio_activo: boolean;
  personalizacion_actualizada_en: string | null;
  contenido_actualizado_en: string | null;
  origen_personalizacion: Tienda['origen_personalizacion'];
  creado_en: string;
  actualizado_en: string;
}

/** Lista blanca explícita: un campo nuevo no se publica por accidente. */
export function aOfertaPublica(oferta: Oferta): OfertaPublica {
  return {
    id: oferta.id,
    item_type: oferta.item_type,
    item_id: oferta.item_id,
    sku: oferta.sku,
    producto_nombre: oferta.producto_nombre,
    atributos: oferta.atributos,
    imagen_objeto_id: oferta.imagen_objeto_id,
    imagen_url: oferta.imagen_url,
    precio_venta: oferta.precio_venta,
    stock: oferta.stock,
    estado: oferta.estado,
    creado_en: oferta.creado_en,
  };
}

/** Lista blanca explícita de la tienda pública del storefront. */
export function aTiendaPublica(tienda: Tienda): TiendaPublica {
  return {
    id: tienda.id,
    nombre: tienda.nombre,
    slug: tienda.slug,
    descripcion: tienda.descripcion,
    logo_objeto_id: tienda.logo_objeto_id,
    logo_url: tienda.logo_url ?? null,
    background_objeto_id: tienda.background_objeto_id,
    background_media_type: tienda.background_media_type,
    background_poster_objeto_id: tienda.background_poster_objeto_id,
    background_alt: tienda.background_alt,
    background_overlay: tienda.background_overlay,
    background_position: tienda.background_position,
    background_activo: tienda.background_activo,
    background_url: tienda.background_url ?? null,
    background_poster_url: tienda.background_poster_url ?? null,
    background_movil_objeto_id: tienda.background_movil_objeto_id,
    background_movil_media_type: tienda.background_movil_media_type,
    background_movil_poster_objeto_id: tienda.background_movil_poster_objeto_id,
    background_movil_alt: tienda.background_movil_alt,
    background_movil_activo: tienda.background_movil_activo,
    plantilla: tienda.plantilla,
    tema: tienda.tema,
    hero_titulo: tienda.hero_titulo,
    hero_subtitulo: tienda.hero_subtitulo,
    hero_cta_texto: tienda.hero_cta_texto,
    hero_cta_url: tienda.hero_cta_url,
    seo_titulo: tienda.seo_titulo,
    seo_descripcion: tienda.seo_descripcion,
    seo_imagen_objeto_id: tienda.seo_imagen_objeto_id,
    seo_imagen_url: tienda.seo_imagen_url ?? null,
    secciones: tienda.secciones,
    sobre_nosotros: tienda.sobre_nosotros,
    politicas: tienda.politicas,
    redes: tienda.redes,
    anuncio_texto: tienda.anuncio_texto,
    anuncio_enlace: tienda.anuncio_enlace,
    anuncio_activo: tienda.anuncio_activo,
    personalizacion_actualizada_en: tienda.personalizacion_actualizada_en,
    contenido_actualizado_en: tienda.contenido_actualizado_en,
    origen_personalizacion: tienda.origen_personalizacion,
    creado_en: tienda.creado_en,
    actualizado_en: tienda.actualizado_en,
  };
}
