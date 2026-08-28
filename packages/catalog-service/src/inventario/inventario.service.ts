import { Injectable } from '@nestjs/common';
import { PgService, Money } from '@core/shared';

export interface LineaInventario {
  sku: string;
  nombre: string;
  stock: number;           // stock_fisico
  disponible: number;      // stock_fisico - reservado
  reservado: number;       // reservas activas
  estado: string;
  precio_base: string;
  valor_disponible_cents: number;
  valor_total_cents: number;
}

/**
 * H-06 / GET /api/v1/admin/inventario: niveles de stock de la bodega (desde proyección CQRS).
 */
@Injectable()
export class InventarioService {
  constructor(private readonly pg: PgService) {}

  async listar(): Promise<LineaInventario[]> {
    const filas = await this.pg.query<{
      sku: string;
      nombre: string;
      stock_fisico: number;
      reservado: number;
      disponible: number;
      estado: string;
      precio_base_cents: number;
      valor_disponible_cents: number;
      valor_total_cents: number;
    }>(
      `SELECT sku, nombre, stock_fisico, reservado, disponible, estado,
              precio_base_cents, valor_disponible_cents, valor_total_cents
       FROM catalog.stock_vista
       ORDER BY estado DESC, nombre ASC`,
    );
    return filas.map((f) => ({
      sku: f.sku,
      nombre: f.nombre,
      stock: f.stock_fisico,
      disponible: f.disponible,
      reservado: f.reservado,
      estado: f.estado,
      precio_base: Money.desdeCentavos(f.precio_base_cents).string(),
      reservas_activas: f.reservado,
      valor_disponible_cents: f.valor_disponible_cents,
      valor_total_cents: f.valor_total_cents,
    }));
  }
}