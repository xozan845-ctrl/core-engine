/**
 * Puerto de almacenamiento de objetos (R-SB-6): la logica de negocio no debe
 * acoplarse al SDK del proveedor. La implementacion concreta (MinIO/S3/
 * Supabase Storage) vive en el servicio que la usa y se inyecta a traves de
 * esta interfaz, de modo que las pruebas unitarias usen dobles y cambiar de
 * proveedor no toque el dominio.
 */

export interface OpcionesUrlSubida {
  bucket: string;
  objeto: string;
  /** Content-Type que el cliente debe declarar en la subida (queda firmado). */
  content_type: string;
  expira_segundos: number;
}

/** Token de inyeccion del puerto (lo provee el servicio con su implementacion). */
export const ALMACEN_OBJETOS = 'ALMACEN_OBJETOS';

export interface ObjetoEnAlmacen {
  bucket: string;
  objeto: string;
  tamano_bytes: number;
  content_type?: string;
  etag?: string;
}

import type { Readable } from 'stream';

/**
 * Objeto listo para servir por streaming (imágenes públicas). El stream es el
 * cuerpo del objeto; el servicio que lo consume decide los headers HTTP.
 */
export interface ObjetoLeido {
  stream: Readable;
  content_type?: string;
  tamano_bytes: number;
  etag?: string;
}

export interface AlmacenObjetos {
  /** Crea el bucket si no existe (idempotente). */
  asegurarBucket(bucket: string): Promise<void>;
  /** true si el bucket existe en el proveedor. */
  existeBucket(bucket: string): Promise<boolean>;
  /** URL prefirmada de subida (PUT) con validez limitada. */
  urlSubida(opciones: OpcionesUrlSubida): Promise<string>;
  /** URL prefirmada de descarga (GET) con validez limitada. */
  urlDescarga(bucket: string, objeto: string, expira_segundos: number): Promise<string>;
  /** Metadatos del objeto en el proveedor; `null` si no existe. */
  estadisticas(bucket: string, objeto: string): Promise<ObjetoEnAlmacen | null>;
  /** Abre el objeto para servirlo por streaming; `null` si no existe. */
  leer(bucket: string, objeto: string): Promise<ObjetoLeido | null>;
  /** Elimina el objeto del bucket (no falla si ya no existe). */
  eliminar(bucket: string, objeto: string): Promise<void>;
}
