import { Inject, Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  ALMACEN_OBJETOS,
  AlmacenObjetos,
  ConflictError,
  DomainError,
  EVENTOS,
  ForbiddenError,
  NotFoundError,
  ObjetoAlmacenadoData,
  ObjetoEliminadoData,
  OutboxService,
  Pagina,
  PgService,
  ROLES,
  UsuarioContexto,
  crearPagina,
  parsearPaginacion,
} from '@core/shared';
import type { PoolClient } from 'pg';
import { STORAGE_CONFIG, StorageConfig } from '../config/storage.config';
import { BucketsService } from '../buckets/buckets.service';

export type EstadoObjeto = 'pendiente' | 'activo' | 'eliminado';

export interface ObjetoStorage {
  id: string;
  tenant_id: string;
  bucket: string;
  objeto: string;
  nombre: string;
  content_type: string;
  carpeta: string;
  descripcion: string;
  etiquetas: Record<string, string>;
  tamano_bytes: number;
  etag: string | null;
  estado: EstadoObjeto;
  subido_por: string | null;
  creado_en: string;
  confirmado_en: string | null;
  eliminado_en: string | null;
}

export interface SolicitudSubida {
  objeto_id: string;
  bucket: string;
  objeto: string;
  url_subida: string;
  metodo: 'PUT';
  headers: Record<string, string>;
  expira_en: string;
  max_bytes: number;
}

export interface UrlDescarga {
  url_descarga: string;
  expira_en: string;
  nombre: string;
  content_type: string;
  tamano_bytes: number;
}

export interface DatosSubida {
  nombre: unknown;
  content_type: unknown;
  tamano_bytes: unknown;
  carpeta?: unknown;
  descripcion?: unknown;
  etiquetas?: unknown;
}

interface ObjetoRow {
  id: string;
  tenant_id: string;
  bucket: string;
  objeto: string;
  nombre_original: string;
  content_type: string;
  carpeta: string;
  descripcion: string;
  etiquetas: Record<string, string> | null;
  tamano_bytes: string | number;
  etag: string | null;
  estado: string;
  subido_por: string | null;
  creado_en: string;
  confirmado_en: string | null;
  eliminado_en: string | null;
}

const COLUMNAS = `id, tenant_id, bucket, objeto, nombre_original, content_type, carpeta,
  descripcion, etiquetas, tamano_bytes, etag, estado, subido_por, creado_en,
  confirmado_en, eliminado_en`;

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Almacenamiento de objetos por tenant (ADR-24): solicita URLs prefirmadas de
 * subida/descarga contra MinIO y mantiene la metadata, las cuotas y los eventos
 * del dominio. Los bytes no pasan por el servicio.
 */
@Injectable()
export class ObjetosService {
  constructor(
    private readonly pg: PgService,
    private readonly buckets: BucketsService,
    private readonly outbox: OutboxService,
    @Inject(ALMACEN_OBJETOS) private readonly almacen: AlmacenObjetos,
    @Inject(STORAGE_CONFIG) private readonly config: StorageConfig,
  ) {}

  /** Paso 1: valida cuota/limites y devuelve la URL prefirmada de subida. */
  async solicitarSubida(
    tenantId: string,
    usuarioId: string | undefined,
    datos: DatosSubida,
  ): Promise<SolicitudSubida> {
    const nombre = this.sanearNombre(datos.nombre);
    const content_type = String(datos.content_type ?? '')
      .trim()
      .toLowerCase();
    if (!this.config.contentTypesPermitidos.includes(content_type)) {
      throw new DomainError(
        'CONTENT_TYPE_NO_PERMITIDO',
        `El tipo "${content_type}" no esta permitido.`,
        { permitidos: this.config.contentTypesPermitidos },
      );
    }
    const tamano_bytes = Number(datos.tamano_bytes);
    if (!Number.isInteger(tamano_bytes) || tamano_bytes <= 0) {
      throw new DomainError('TAMANO_INVALIDO', 'El tamano debe ser un entero positivo en bytes.');
    }
    if (tamano_bytes > this.config.maxObjetoBytes) {
      throw new DomainError(
        'OBJETO_DEMASIADO_GRANDE',
        `El archivo excede el maximo de ${this.config.maxObjetoBytes} bytes.`,
        { max_bytes: this.config.maxObjetoBytes },
      );
    }

    const estado = await this.buckets.asegurar(tenantId);
    this.buckets.verificarCuota(estado, tamano_bytes);

    const id = randomUUID();
    const carpeta = this.sanearCarpeta(datos.carpeta);
    const objeto = `${carpeta}/${id}/${nombre}`;
    const etiquetas = this.sanearEtiquetas(datos.etiquetas);
    // El gateway inyecta el id real del usuario (uuid); si llega otra cosa se
    // guarda sin autor en vez de romper la insercion.
    const autor = usuarioId && UUID.test(usuarioId) ? usuarioId : null;
    const fila = await this.pg.queryOne<ObjetoRow>(
      `INSERT INTO storage.objetos
         (id, tenant_id, bucket, objeto, nombre_original, content_type, carpeta,
          descripcion, etiquetas, tamano_bytes, estado, subido_por, creado_en)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb, $10, 'pendiente', $11, NOW())
       RETURNING ${COLUMNAS}`,
      [
        id,
        tenantId,
        estado.bucket,
        objeto,
        nombre,
        content_type,
        carpeta,
        String(datos.descripcion ?? '').slice(0, 500),
        JSON.stringify(etiquetas),
        tamano_bytes,
        autor,
      ],
    );
    if (!fila) throw new ConflictError('No se pudo registrar la solicitud de subida.');

    const url_subida = await this.almacen.urlSubida({
      bucket: estado.bucket,
      objeto,
      content_type,
      expira_segundos: this.config.urlTtlSegundos,
    });
    return {
      objeto_id: fila.id,
      bucket: estado.bucket,
      objeto,
      url_subida,
      metodo: 'PUT',
      headers: { 'Content-Type': content_type },
      expira_en: this.expiracion(),
      max_bytes: this.config.maxObjetoBytes,
    };
  }

  /** Paso 2: verifica contra MinIO el objeto subido y lo activa (cuenta cuota). */
  async confirmar(tenantId: string, id: string): Promise<ObjetoStorage> {
    const fila = await this.obtenerFila(tenantId, id);
    if (fila.estado === 'eliminado') {
      throw new ConflictError('El objeto fue eliminado y no puede confirmarse.');
    }
    if (fila.estado === 'activo') return this.serializar(fila);

    const info = await this.almacen.estadisticas(fila.bucket, fila.objeto);
    if (!info) {
      throw new NotFoundError('Objeto en el almacenamiento', fila.objeto);
    }
    if (info.tamano_bytes <= 0) {
      await this.almacen.eliminar(fila.bucket, fila.objeto);
      throw new DomainError('OBJETO_VACIO', 'El objeto subido esta vacio.');
    }
    if (info.tamano_bytes > this.config.maxObjetoBytes) {
      await this.almacen.eliminar(fila.bucket, fila.objeto);
      throw new DomainError(
        'OBJETO_DEMASIADO_GRANDE',
        `El archivo excede el maximo de ${this.config.maxObjetoBytes} bytes.`,
        { max_bytes: this.config.maxObjetoBytes },
      );
    }
    const content_type = (info.content_type ?? fila.content_type).trim().toLowerCase();
    if (!this.config.contentTypesPermitidos.includes(content_type)) {
      await this.almacen.eliminar(fila.bucket, fila.objeto);
      throw new DomainError(
        'CONTENT_TYPE_NO_PERMITIDO',
        `El objeto subido tiene un tipo no permitido ("${content_type}").`,
        { permitidos: this.config.contentTypesPermitidos },
      );
    }

    // El tamano real manda: revalida la cuota antes de contabilizarlo.
    const estado = await this.buckets.estado(tenantId);
    this.buckets.verificarCuota(estado, Math.max(0, info.tamano_bytes - Number(fila.tamano_bytes)));

    const confirmado = await this.pg.transaccion(async (client) => {
      const r = await client.query<ObjetoRow>(
        `UPDATE storage.objetos
            SET estado = 'activo', tamano_bytes = $3, content_type = $4, etag = $5,
                confirmado_en = NOW()
          WHERE tenant_id = $1 AND id = $2 AND estado = 'pendiente'
          RETURNING ${COLUMNAS}`,
        [tenantId, id, info.tamano_bytes, content_type, info.etag ?? null],
      );
      if (r.rowCount === 0) return null;
      await this.buckets.sumarUsoEnTransaccion(client, tenantId, info.tamano_bytes);
      await this.emitirSubido(client, r.rows[0]);
      return r.rows[0];
    });

    // Carrera: otra confirmacion gano; el estado actual ya es valido.
    if (!confirmado) return this.obtener(tenantId, id);
    return this.serializar(confirmado);
  }

  async listar(
    tenantId: string,
    filtros: { q?: unknown; carpeta?: unknown; estado?: unknown; pagina?: unknown; limite?: unknown },
  ): Promise<Pagina<ObjetoStorage>> {
    const { pagina, limite } = parsearPaginacion(filtros as Record<string, unknown>);
    const condiciones: string[] = ['tenant_id = $1'];
    const params: unknown[] = [tenantId];
    const estado = filtros.estado ? String(filtros.estado) : undefined;
    if (estado) {
      params.push(estado);
      condiciones.push(`estado = $${params.length}`);
    } else {
      condiciones.push(`estado <> 'eliminado'`);
    }
    if (filtros.carpeta) {
      params.push(String(filtros.carpeta).toLowerCase());
      condiciones.push(`carpeta = $${params.length}`);
    }
    if (filtros.q) {
      params.push(`%${String(filtros.q).toLowerCase()}%`);
      condiciones.push(`LOWER(nombre_original) LIKE $${params.length}`);
    }
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(`SELECT COUNT(*)::int AS n FROM storage.objetos ${where}`, params)
      .then((r) => r?.n ?? 0);
    const filas = await this.pg.query<ObjetoRow>(
      `SELECT ${COLUMNAS} FROM storage.objetos ${where}
       ORDER BY creado_en DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, (pagina - 1) * limite],
    );
    return crearPagina(filas.map((f) => this.serializar(f)), total, { pagina, limite });
  }

  async obtener(tenantId: string, id: string): Promise<ObjetoStorage> {
    return this.serializar(await this.obtenerFila(tenantId, id));
  }

  /** URL prefirmada de descarga (GET) con validez limitada. */
  async urlDescarga(tenantId: string, id: string): Promise<UrlDescarga> {
    const fila = await this.obtenerFila(tenantId, id);
    if (fila.estado === 'eliminado') throw new NotFoundError('Objeto', id);
    const url_descarga = await this.almacen.urlDescarga(
      fila.bucket,
      fila.objeto,
      this.config.urlTtlSegundos,
    );
    return {
      url_descarga,
      expira_en: this.expiracion(),
      nombre: fila.nombre_original,
      content_type: fila.content_type,
      tamano_bytes: Number(fila.tamano_bytes),
    };
  }

  /** Baja logica + borrado en MinIO + liberacion de cuota (idempotente). */
  async eliminar(tenantId: string, id: string, usuario?: UsuarioContexto | null): Promise<void> {
    const fila = await this.obtenerFila(tenantId, id);
    if (usuario && usuario.rol !== ROLES.ADMIN) {
      if (fila.subido_por && fila.subido_por !== usuario.user_id) {
        throw new ForbiddenError('Solo el autor de la subida o un administrador pueden eliminarlo.');
      }
    }
    if (fila.estado === 'eliminado') return;

    await this.almacen.eliminar(fila.bucket, fila.objeto);
    await this.pg.transaccion(async (client) => {
      const r = await client.query<ObjetoRow>(
        `UPDATE storage.objetos SET estado = 'eliminado', eliminado_en = NOW()
          WHERE tenant_id = $1 AND id = $2 AND estado <> 'eliminado'
          RETURNING ${COLUMNAS}`,
        [tenantId, id],
      );
      if (r.rowCount === 0) return;
      if (fila.estado === 'activo') {
        await this.buckets.restarUsoEnTransaccion(client, tenantId, Number(fila.tamano_bytes));
      }
      await this.emitirEliminado(client, r.rows[0]);
    });
  }

  private async obtenerFila(tenantId: string, id: string): Promise<ObjetoRow> {
    if (!UUID.test(id)) throw new NotFoundError('Objeto', id);
    const fila = await this.pg.queryOne<ObjetoRow>(
      `SELECT ${COLUMNAS} FROM storage.objetos WHERE tenant_id = $1 AND id = $2`,
      [tenantId, id],
    );
    if (!fila) throw new NotFoundError('Objeto', id);
    return fila;
  }

  private async emitirSubido(client: PoolClient, fila: ObjetoRow): Promise<void> {
    const evento: ObjetoAlmacenadoData = {
      objeto_id: fila.id,
      tenant_id: fila.tenant_id,
      bucket: fila.bucket,
      objeto: fila.objeto,
      nombre: fila.nombre_original,
      content_type: fila.content_type,
      tamano_bytes: Number(fila.tamano_bytes),
      ...(fila.subido_por ? { subido_por: fila.subido_por } : {}),
    };
    await this.outbox.insertarEnTransaccion(client, EVENTOS.STORAGE_OBJETO_SUBIDO, evento);
  }

  private async emitirEliminado(client: PoolClient, fila: ObjetoRow): Promise<void> {
    const evento: ObjetoEliminadoData = {
      objeto_id: fila.id,
      tenant_id: fila.tenant_id,
      bucket: fila.bucket,
      objeto: fila.objeto,
      nombre: fila.nombre_original,
      tamano_bytes: Number(fila.tamano_bytes),
    };
    await this.outbox.insertarEnTransaccion(client, EVENTOS.STORAGE_OBJETO_ELIMINADO, evento);
  }

  private sanearNombre(valor: unknown): string {
    const base = String(valor ?? '')
      .split(/[\\/]/)
      .pop();
    const limpio = String(base ?? '')
      .replace(/[^A-Za-z0-9._-]+/g, '_')
      .replace(/^[._]+/, '')
      .slice(0, 180);
    return limpio || 'archivo';
  }

  private sanearCarpeta(valor: unknown): string {
    const limpio = String(valor ?? '')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9._-]+/g, '-')
      .replace(/^[-.]+|[-.]+$/g, '')
      .slice(0, 40);
    return limpio || 'general';
  }

  private sanearEtiquetas(valor: unknown): Record<string, string> {
    if (!valor || typeof valor !== 'object' || Array.isArray(valor)) return {};
    const entradas = Object.entries(valor as Record<string, unknown>).slice(0, 20);
    const salida: Record<string, string> = {};
    for (const [clave, dato] of entradas) {
      const k = String(clave).trim().slice(0, 40);
      if (!k) continue;
      salida[k] = String(dato ?? '').slice(0, 200);
    }
    return salida;
  }

  private expiracion(): string {
    return new Date(Date.now() + this.config.urlTtlSegundos * 1000).toISOString();
  }

  private serializar(fila: ObjetoRow): ObjetoStorage {
    return {
      id: fila.id,
      tenant_id: fila.tenant_id,
      bucket: fila.bucket,
      objeto: fila.objeto,
      nombre: fila.nombre_original,
      content_type: fila.content_type,
      carpeta: fila.carpeta,
      descripcion: fila.descripcion,
      etiquetas: fila.etiquetas ?? {},
      tamano_bytes: Number(fila.tamano_bytes),
      etag: fila.etag ?? null,
      estado:
        fila.estado === 'activo' ? 'activo' : fila.estado === 'eliminado' ? 'eliminado' : 'pendiente',
      subido_por: fila.subido_por ?? null,
      creado_en: fila.creado_en,
      confirmado_en: fila.confirmado_en ?? null,
      eliminado_en: fila.eliminado_en ?? null,
    };
  }
}
