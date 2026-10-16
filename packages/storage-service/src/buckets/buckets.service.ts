import { Inject, Injectable } from '@nestjs/common';
import type { PoolClient } from 'pg';
import {
  ALMACEN_OBJETOS,
  AlmacenObjetos,
  DomainError,
  NotFoundError,
  Pagina,
  PgService,
  crearPagina,
  parsearPaginacion,
} from '@core/shared';
import { STORAGE_CONFIG, StorageConfig } from '../config/storage.config';

export type EstadoAlmacen = 'activo' | 'suspendido';

export interface EstadoBucket {
  tenant_id: string;
  bucket: string;
  cuota_bytes: number;
  usado_bytes: number;
  disponible_bytes: number;
  objetos: number;
  estado: EstadoAlmacen;
  creado_en: string;
  actualizado_en: string;
}

interface BucketRow {
  tenant_id: string;
  bucket: string;
  cuota_bytes: string | number;
  usado_bytes: string | number;
  estado: string;
  creado_en: string;
  actualizado_en: string;
  objetos?: number;
}

const COLUMNAS_BUCKET = `b.tenant_id, b.bucket, b.cuota_bytes, b.usado_bytes, b.estado,
  b.creado_en, b.actualizado_en,
  (SELECT COUNT(*)::int FROM storage.objetos o
    WHERE o.tenant_id = b.tenant_id AND o.estado = 'activo') AS objetos`;

/**
 * Buckets y cuotas por tenant (ADR-24): un bucket por organizacion, con su
 * fila de control en `storage.buckets` que guarda cuota, uso y estado.
 */
@Injectable()
export class BucketsService {
  constructor(
    private readonly pg: PgService,
    @Inject(ALMACEN_OBJETOS) private readonly almacen: AlmacenObjetos,
    @Inject(STORAGE_CONFIG) private readonly config: StorageConfig,
  ) {}

  /** Nombre del bucket del tenant (S3 exige minusculas: `t-<uuid>`). */
  nombreBucket(tenantId: string): string {
    return `${this.config.prefijoBucket}${tenantId}`.toLowerCase();
  }

  /** Asegura bucket fisico + fila de control (idempotente). */
  async asegurar(tenantId: string): Promise<EstadoBucket> {
    const bucket = this.nombreBucket(tenantId);
    await this.almacen.asegurarBucket(bucket);
    await this.pg.query(
      `INSERT INTO storage.buckets (tenant_id, bucket, cuota_bytes, creado_en, actualizado_en)
       VALUES ($1, $2, $3, NOW(), NOW())
       ON CONFLICT (tenant_id) DO NOTHING`,
      [tenantId, bucket, this.config.cuotaDefaultBytes],
    );
    return this.estado(tenantId);
  }

  async estado(tenantId: string): Promise<EstadoBucket> {
    const fila = await this.pg.queryOne<BucketRow>(
      `SELECT ${COLUMNAS_BUCKET} FROM storage.buckets b WHERE b.tenant_id = $1`,
      [tenantId],
    );
    if (!fila) throw new NotFoundError('Bucket del tenant', tenantId);
    return this.serializar(fila);
  }

  async listar(pagina?: unknown, limite?: unknown): Promise<Pagina<EstadoBucket>> {
    const { pagina: p, limite: l } = parsearPaginacion({ pagina, limite });
    const total = await this.pg
      .queryOne<{ n: number }>(`SELECT COUNT(*)::int AS n FROM storage.buckets`)
      .then((r) => r?.n ?? 0);
    const filas = await this.pg.query<BucketRow>(
      `SELECT ${COLUMNAS_BUCKET} FROM storage.buckets b
       ORDER BY b.creado_en DESC LIMIT $1 OFFSET $2`,
      [l, (p - 1) * l],
    );
    return crearPagina(filas.map((f) => this.serializar(f)), total, { pagina: p, limite: l });
  }

  /** Cuota del tenant (admin). Nunca por debajo del uso actual. */
  async actualizarCuota(tenantId: string, cuotaBytes: number): Promise<EstadoBucket> {
    if (!Number.isInteger(cuotaBytes) || cuotaBytes <= 0) {
      throw new DomainError('CUOTA_INVALIDA', 'La cuota debe ser un entero positivo en bytes.');
    }
    const actual = await this.estado(tenantId);
    if (cuotaBytes < actual.usado_bytes) {
      throw new DomainError(
        'CUOTA_MENOR_AL_USO',
        `La cuota no puede ser menor al uso actual (${actual.usado_bytes} bytes).`,
        { usado_bytes: actual.usado_bytes },
      );
    }
    await this.pg.query(
      `UPDATE storage.buckets SET cuota_bytes = $2, actualizado_en = NOW() WHERE tenant_id = $1`,
      [tenantId, cuotaBytes],
    );
    return this.estado(tenantId);
  }

  /** Suspende o reactiva el almacenamiento de un tenant (admin). */
  async cambiarEstado(tenantId: string, estado: EstadoAlmacen): Promise<EstadoBucket> {
    await this.estado(tenantId);
    await this.pg.query(
      `UPDATE storage.buckets SET estado = $2, actualizado_en = NOW() WHERE tenant_id = $1`,
      [tenantId, estado],
    );
    return this.estado(tenantId);
  }

  /** Valida que el tenant pueda recibir `bytesNuevos` sin exceder su cuota. */
  verificarCuota(estado: EstadoBucket, bytesNuevos: number): void {
    if (estado.estado !== 'activo') {
      throw new DomainError(
        'ALMACENAMIENTO_SUSPENDIDO',
        'El almacenamiento del tenant esta suspendido.',
      );
    }
    if (estado.usado_bytes + bytesNuevos > estado.cuota_bytes) {
      throw new DomainError(
        'CUOTA_EXCEDIDA',
        'La operacion excede la cuota de almacenamiento del tenant.',
        {
          cuota_bytes: estado.cuota_bytes,
          usado_bytes: estado.usado_bytes,
          solicitado_bytes: bytesNuevos,
        },
      );
    }
  }

  async sumarUsoEnTransaccion(client: PoolClient, tenantId: string, bytes: number): Promise<void> {
    await client.query(
      `UPDATE storage.buckets SET usado_bytes = usado_bytes + $2, actualizado_en = NOW()
       WHERE tenant_id = $1`,
      [tenantId, bytes],
    );
  }

  async restarUsoEnTransaccion(client: PoolClient, tenantId: string, bytes: number): Promise<void> {
    await client.query(
      `UPDATE storage.buckets SET usado_bytes = GREATEST(0, usado_bytes - $2), actualizado_en = NOW()
       WHERE tenant_id = $1`,
      [tenantId, bytes],
    );
  }

  /** Recalcula el uso desde la metadata (mantenimiento idempotente, servicio->servicio). */
  async recalcularUso(tenantId: string): Promise<EstadoBucket> {
    await this.pg.query(
      `UPDATE storage.buckets b
          SET usado_bytes = COALESCE((
                SELECT SUM(o.tamano_bytes) FROM storage.objetos o
                 WHERE o.tenant_id = b.tenant_id AND o.estado = 'activo'), 0),
              actualizado_en = NOW()
        WHERE b.tenant_id = $1`,
      [tenantId],
    );
    return this.estado(tenantId);
  }

  private serializar(fila: BucketRow): EstadoBucket {
    const cuota = Number(fila.cuota_bytes);
    const usado = Number(fila.usado_bytes);
    return {
      tenant_id: fila.tenant_id,
      bucket: fila.bucket,
      cuota_bytes: cuota,
      usado_bytes: usado,
      disponible_bytes: Math.max(0, cuota - usado),
      objetos: Number(fila.objetos ?? 0),
      estado: fila.estado === 'suspendido' ? 'suspendido' : 'activo',
      creado_en: fila.creado_en,
      actualizado_en: fila.actualizado_en,
    };
  }
}
