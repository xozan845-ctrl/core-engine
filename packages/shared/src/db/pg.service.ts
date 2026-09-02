import { Injectable, OnModuleDestroy, OnModuleInit, Optional } from '@nestjs/common';
import { Pool, PoolClient } from 'pg';
import { Logger } from '../logging/logger';

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

  constructor(@Optional() private readonly config?: Partial<PgConfig>) {}

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

  query<T = Record<string, unknown>>(text: string, params: unknown[] = []): Promise<T[]> {
    if (!this.pool) {
      return Promise.reject(new Error('Pool no inicializado.'));
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
}