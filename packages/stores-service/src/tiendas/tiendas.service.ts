import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import { PgService, DomainError, ConflictError, NotFoundError } from '@core/shared';

export interface Tienda {
  id: string;
  vendedor_id: string;
  nombre: string;
  descripcion: string;
  creado_en: string;
}

/**
 * Tiendas: espacio virtual de cada vendedor (Tabla 11). Una tienda por vendedor.
 */
@Injectable()
export class TiendasService {
  constructor(private readonly pg: PgService) {}

  async deVendedor(vendedorId: string): Promise<Tienda | null> {
    return this.pg.queryOne<Tienda>(
      `SELECT id, vendedor_id, nombre, descripcion, creado_en
       FROM stores.tiendas WHERE vendedor_id = $1`,
      [vendedorId],
    );
  }

  async encontrarPorId(id: string): Promise<Tienda | null> {
    return this.pg.queryOne<Tienda>(
      `SELECT id, vendedor_id, nombre, descripcion, creado_en
       FROM stores.tiendas WHERE id = $1`,
      [id],
    );
  }

  async crear(vendedorId: string, nombre: string, descripcion?: string): Promise<Tienda> {
    const nombreFinal = nombre.trim();
    if (nombreFinal.length < 2 || nombreFinal.length > 100) {
      throw new DomainError('NOMBRE_INVALIDO', 'El nombre de la tienda debe tener entre 2 y 100 caracteres.');
    }
    const existente = await this.deVendedor(vendedorId);
    if (existente) {
      throw new ConflictError('Este vendedor ya tiene una tienda.');
    }
    const fila = await this.pg.queryOne<Tienda>(
      `INSERT INTO stores.tiendas (id, vendedor_id, nombre, descripcion)
       VALUES ($1, $2, $3, $4) RETURNING id, vendedor_id, nombre, descripcion, creado_en`,
      [randomUUID(), vendedorId, nombreFinal, (descripcion ?? '').trim()],
    );
    if (!fila) throw new DomainError('TIENDA_FALLIDA', 'No se pudo crear la tienda.');
    return fila;
  }
}