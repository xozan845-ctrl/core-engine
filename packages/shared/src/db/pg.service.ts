import { Injectable, OnModuleDestroy, OnModuleInit, Optional } from '@nestjs/common';
import { Pool, PoolClient } from 'pg';
import { Logger, contextoActual } from '../logging/logger';

export interface PgConfig {
  /** URL de conexion, ej: postgres://user:pass@host:5432/core_engine */
  url: string;
}

/**
 * Pool de conexiones con soporte de transacciones. Cada servicio usa su schema
 * de dominio; la configuracion llega por variable de entorno.
 */
@Injectable()
export class PgService implements OnModuleInit, OnModuleDestroy {
  private pool?: Pool;
  private readonly logger = Logger.create('pg');
  /** Activa SET ROLE + claims para RLS (R-DB-2/R-SB-3). */
  private readonly rlsHabilitado = (process.env.DB_RLS_ENABLED ?? 'false').toLowerCase() === 'true';
  /** Rol BYPASSRLS para background (outbox, consumidores, seed). */
  private readonly rolServicio = this.validarRol(process.env.DB_SERVICE_ROLE ?? 'core_engine_service');

  constructor(@Optional() private readonly config?: Partial<PgConfig>) {}

  /** El nombre del rol se interpola en SET ROLE: solo identificadores simples. */
  private validarRol(rol: string): string {
    if (!/^[a-z_][a-z0-9_]*$/.test(rol)) {
      throw new Error(`DB_SERVICE_ROLE invalido: ${rol}`);
    }
    return rol;
  }

  async onModuleInit(): Promise<void> {
    const url = this.config?.url ?? process.env.DATABASE_URL ?? '';
    if (!url) {
      throw new Error('DATABASE_URL no esta definida.');
    }
    this.pool = new Pool({ connectionString: url, max: 10 });
    this.pool.on('error', (err) => this.logger.error({ msg: 'Error en pool de conexiones', err: err.message }));
    await this.pool.query('SELECT 1');
    this.logger.info({ msg: 'Conexion a PostgreSQL establecida' });
  }

  async onModuleDestroy(): Promise<void> {
    if (this.pool) {
      await this.pool.end();
      this.pool = undefined;
    }
  }

  async query<T = Record<string, unknown>>(text: string, params: unknown[] = []): Promise<T[]> {
    if (!this.pool) {
      return Promise.reject(new Error('Pool no inicializado.'));
    }
    const ctx = contextoActual();
    if (this.rlsHabilitado || ctx?.usuario_id) {
      // Una transaccion por consulta para que SET LOCAL ROLE y set_config(...,
      // true) apliquen solo a esta operacion y protejan RLS (R-DB-2).
      return this.transaccion(async (client) => {
        const r = await client.query(text, params);
        return r.rows as T[];
      });
    }
    return this.pool.query(text, params).then((r) => r.rows as T[]);
  }

  queryOne<T = Record<string, unknown>>(text: string, params: unknown[] = []): Promise<T | null> {
    return this.query<T>(text, params).then((rows) => rows[0] ?? null);
  }

  async transaccion<T>(fn: (client: PoolClient) => Promise<T>): Promise<T> {
    if (!this.pool) {
      throw new Error('Pool no inicializado.');
    }
    const client = await this.pool.connect();
    try {
      await client.query('BEGIN');
      await this.configurarSesion(client);
      const resultado = await fn(client);
      await client.query('COMMIT');
      return resultado;
    } catch (err) {
      await client.query('ROLLBACK');
      throw err;
    } finally {
      client.release();
    }
  }

  /**
   * Configura la sesion transaccional:
   * - con usuario: SET ROLE authenticated + claims (sub/rol/tenant) para RLS;
   * - sin usuario: SET ROLE del servicio (BYPASSRLS) para background/seed;
   * - con RLS desactivado: conserva el comportamiento historico (solo claims).
   */
  private async configurarSesion(client: PoolClient): Promise<void> {
    const ctx = contextoActual();
    if (this.rlsHabilitado) {
      await client.query(
        ctx?.usuario_id ? 'SET LOCAL ROLE authenticated' : `SET LOCAL ROLE ${this.rolServicio}`,
      );
    }
    if (ctx?.usuario_id) {
      // Inyectar claims JWT en la sesion transaccional para RLS (R-DB-2/R-SB-3):
      // incluye el tenant para que las politicas aíslen por organizacion.
      await client.query(`SELECT set_config('request.jwt.claims', $1, true)`, [
        JSON.stringify({
          sub: ctx.usuario_id,
          rol: ctx.rol ?? 'anon',
          ...(ctx.tenant_id ? { tenant_id: ctx.tenant_id } : {}),
        }),
      ]);
    }
  }
}