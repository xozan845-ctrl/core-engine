import type { ImagenProducto, Producto } from './productos.service';

/**
 * Proyección pública del catálogo (R-DS-3 / R-C-3).
 *
 * Es una lista blanca explícita: un campo nuevo en `Producto` NO se publica
 * hasta que se agregue aquí. Nunca incluye costo, stock exacto, umbrales de
 * inventario, código de barras, ubicación ni clasificación interna. El stock
 * exacto se reemplaza por `disponible`, que es lo único que necesita el
 * storefront.
 */

export interface ImagenPublica {
  id: string;
  objeto_id: string;
  url: string;
  orden: number;
  es_principal: boolean;
}

export interface VariantePublica {
  id: string;
  producto_padre_id: string | null;
  sku: string;
  nombre: string;
  atributos: Record<string, unknown>;
  precio_base: string;
  estado: 'disponible' | 'agotado';
  disponible: boolean;
  imagenes: ImagenPublica[];
  creado_en: string;
}

export interface ProductoPublico {
  id: string;
  empresa_id: string;
  sku: string;
  nombre: string;
  descripcion: string;
  categoria: string;
  precio_base: string;
  estado: 'disponible' | 'agotado';
  disponible: boolean;
  marca: string;
  unidad: string;
  permite_fracciones: boolean;
  unidad_compra: string;
  contenido_cantidad: number;
  contenido_unidad: string;
  tipo_precio: 'fijo' | 'escala';
  garantia_meses: number | null;
  tiene_variantes: boolean;
  imagenes: ImagenPublica[];
  producto_padre_id: string | null;
  atributos: Record<string, unknown>;
  precio_desde: string;
  creado_en: string;
  actualizado_en: string;
  variantes?: VariantePublica[];
}

function aImagenPublica(imagen: ImagenProducto): ImagenPublica {
  return {
    id: imagen.id,
    objeto_id: imagen.objeto_id,
    url: imagen.url,
    orden: imagen.orden,
    es_principal: imagen.es_principal,
  };
}

function aVariantePublica(variante: Producto): VariantePublica {
  return {
    id: variante.id,
    producto_padre_id: variante.producto_padre_id,
    sku: variante.sku,
    nombre: variante.nombre,
    atributos: variante.atributos,
    precio_base: variante.precio_base,
    estado: variante.estado,
    disponible: variante.estado === 'disponible',
    imagenes: (variante.imagenes ?? []).map(aImagenPublica),
    creado_en: variante.creado_en,
  };
}

export function aProductoPublico(producto: Producto): ProductoPublico {
  return {
    id: producto.id,
    empresa_id: producto.empresa_id,
    sku: producto.sku,
    nombre: producto.nombre,
    descripcion: producto.descripcion,
    categoria: producto.categoria,
    precio_base: producto.precio_base,
    estado: producto.estado,
    disponible: producto.estado === 'disponible',
    marca: producto.marca,
    unidad: producto.unidad,
    permite_fracciones: producto.permite_fracciones,
    unidad_compra: producto.unidad_compra,
    contenido_cantidad: producto.contenido_cantidad,
    contenido_unidad: producto.contenido_unidad,
    tipo_precio: producto.tipo_precio,
    garantia_meses: producto.garantia_meses,
    tiene_variantes: producto.tiene_variantes,
    imagenes: (producto.imagenes ?? []).map(aImagenPublica),
    producto_padre_id: producto.producto_padre_id,
    atributos: producto.atributos,
    precio_desde: producto.precio_desde,
    creado_en: producto.creado_en,
    actualizado_en: producto.actualizado_en,
    ...(producto.variantes ? { variantes: producto.variantes.map(aVariantePublica) } : {}),
  };
}
