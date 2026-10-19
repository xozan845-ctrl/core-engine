import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import { PgService, DomainError } from '@core/shared';

export interface Tienda {
  id: string;
  empresa_id: string;
  propietario_usuario_id: string;
  participacion_porcentaje: number;
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
      `SELECT id, empresa_id, propietario_usuario_id, participacion_porcentaje, vendedor_id, nombre, descripcion, creado_en
       FROM stores.tiendas WHERE vendedor_id = $1`,
      [vendedorId],
    );
  }

  async encontrarPorId(id: string): Promise<Tienda | null> {
    return this.pg.queryOne<Tienda>(
      `SELECT id, empresa_id, propietario_usuario_id, participacion_porcentaje, vendedor_id, nombre, descripcion, creado_en
       FROM stores.tiendas WHERE id = $1`,
      [id],
    );
  }

  async crear(empresaId: string, propietarioId: string, participacion: number, nombre: string, descripcion?: string): Promise<Tienda> {
    const nombreFinal = nombre.trim();
    if (nombreFinal.length < 2 || nombreFinal.length > 100) {
      throw new DomainError('NOMBRE_INVALIDO', 'El nombre de la tienda debe tener entre 2 y 100 caracteres.');
    }
    const fila = await this.pg.queryOne<Tienda>(
      `INSERT INTO stores.tiendas (id, empresa_id, propietario_usuario_id, vendedor_id, participacion_porcentaje, nombre, descripcion)
       VALUES ($1, $2, $3, $3, $4, $5, $6)
       RETURNING id, empresa_id, propietario_usuario_id, participacion_porcentaje, vendedor_id, nombre, descripcion, creado_en`,
      [randomUUID(), empresaId, propietarioId, participacion, nombreFinal, (descripcion ?? '').trim()],
    );
    if (!fila) throw new DomainError('TIENDA_FALLIDA', 'No se pudo crear la tienda.');
    return fila;
  }
}
