import { Injectable } from '@nestjs/common';
import { PgService, Money } from '@core/shared';

export interface LineaInventario {
  empresa_id: string;
  sku: string;
  nombre: string;
  stock: number;           // stock_fisico
  disponible: number;      // catalog.productos.stock, ya neto de reservas
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

  async listar(empresaId?: string): Promise<LineaInventario[]> {
    const filtro = empresaId ? 'WHERE empresa_id = $1' : '';
    const params = empresaId ? [empresaId] : [];
    const filas = await this.pg.query<{
      empresa_id: string;
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
      `SELECT empresa_id, sku, nombre, stock_fisico, reservado, disponible, estado,
              precio_base_cents, valor_disponible_cents, valor_total_cents
       FROM catalog.stock_vista
       ${filtro}
       ORDER BY estado DESC, nombre ASC`,
      params,
    );
    return filas.map((f) => ({
      empresa_id: f.empresa_id,
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
