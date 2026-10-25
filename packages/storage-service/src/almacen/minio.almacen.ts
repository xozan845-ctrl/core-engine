import { Inject, Injectable, OnModuleInit } from '@nestjs/common';
import { Client } from 'minio';
import {
  AlmacenObjetos,
  Logger,
  ObjetoEnAlmacen,
  ObjetoLeido,
  OpcionesUrlSubida,
} from '@core/shared';
import { STORAGE_CONFIG, StorageConfig } from '../config/storage.config';

/**
 * Implementacion de `AlmacenObjetos` sobre MinIO (ADR-24). El dominio depende
 * de la interfaz (R-SB-6), asi que cambiar de proveedor S3-compatible solo toca
 * esta clase y sus dobles de test.
 */
@Injectable()
export class MinioAlmacen implements AlmacenObjetos, OnModuleInit {
  private readonly logger = Logger.create('almacen');
  private readonly cliente: Client;
  private readonly clienteFirma: Client;
  private readonly asegurados = new Set<string>();

  constructor(@Inject(STORAGE_CONFIG) private readonly config: StorageConfig) {
    const credenciales = {
      accessKey: config.accessKey,
      secretKey: config.secretKey,
      region: config.region,
    };
    this.cliente = new Client({
      ...credenciales,
      endPoint: config.endpoint,
      port: config.puerto,
      useSSL: config.usarSsl,
    });
    // Las URLs prefirmadas se calculan en local y llevan el host FIRMADO: deben
    // apuntar al endpoint publico (el que resuelve el cliente final), no al
    // nombre interno del compose. Firmar no abre conexion.
    this.clienteFirma = new Client({
      ...credenciales,
      endPoint: config.publicEndpoint,
      port: config.publicPuerto,
      useSSL: config.publicUsarSsl,
    });
  }

  /**
   * Falla al arrancar si falta la credencial: un servicio de almacenamiento sin
   * acceso al almacen no debe levantar (el `swagger:export` no ejecuta esto).
   */
  onModuleInit(): void {
    if (!this.config.accessKey || !this.config.secretKey) {
      throw new Error('MINIO_ROOT_USER y MINIO_ROOT_PASSWORD son obligatorias.');
    }
  }

  /** Idempotente: crea el bucket la primera vez y lo memoriza por proceso. */
  async asegurarBucket(bucket: string): Promise<void> {
    if (this.asegurados.has(bucket)) return;
    if (!(await this.cliente.bucketExists(bucket))) {
      await this.cliente.makeBucket(bucket);
      this.logger.info({ msg: 'bucket creado', bucket });
    }
    this.asegurados.add(bucket);
  }

  existeBucket(bucket: string): Promise<boolean> {
    return this.cliente.bucketExists(bucket);
  }

  urlSubida(opciones: OpcionesUrlSubida): Promise<string> {
    return this.clienteFirma.presignedPutObject(
      opciones.bucket,
      opciones.objeto,
      opciones.expira_segundos,
    );
  }

  urlDescarga(bucket: string, objeto: string, expira_segundos: number): Promise<string> {
    return this.clienteFirma.presignedGetObject(bucket, objeto, expira_segundos);
  }

  async estadisticas(bucket: string, objeto: string): Promise<ObjetoEnAlmacen | null> {
    try {
      const info = await this.cliente.statObject(bucket, objeto);
      const meta = (info.metaData ?? {}) as Record<string, unknown>;
      const contentType = meta['content-type'];
      return {
        bucket,
        objeto,
        tamano_bytes: info.size,
        ...(typeof contentType === 'string' ? { content_type: contentType } : {}),
        ...(info.etag ? { etag: info.etag } : {}),
      };
    } catch (err) {
      if (this.esNoEncontrado(err)) return null;
      throw err;
    }
  }

  async leer(bucket: string, objeto: string): Promise<ObjetoLeido | null> {
    try {
      const info = await this.cliente.statObject(bucket, objeto);
      const stream = await this.cliente.getObject(bucket, objeto);
      const meta = (info.metaData ?? {}) as Record<string, unknown>;
      const contentType = meta['content-type'];
      return {
        stream,
        tamano_bytes: info.size,
        ...(typeof contentType === 'string' ? { content_type: contentType } : {}),
        ...(info.etag ? { etag: info.etag } : {}),
      };
    } catch (err) {
      if (this.esNoEncontrado(err)) return null;
      throw err;
    }
  }

  async eliminar(bucket: string, objeto: string): Promise<void> {
    try {
      await this.cliente.removeObject(bucket, objeto);
    } catch (err) {
      // Borrar algo que ya no existe es exito para la baja logica (idempotente).
      if (!this.esNoEncontrado(err)) throw err;
    }
  }

  private esNoEncontrado(err: unknown): boolean {
    const codigo = (err as { code?: string } | null)?.code ?? '';
    return ['NoSuchKey', 'NotFound', 'NoSuchBucket'].includes(codigo);
  }
}
