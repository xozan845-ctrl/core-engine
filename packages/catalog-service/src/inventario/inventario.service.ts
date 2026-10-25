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
    const condiciones = [
      'NOT EXISTS (SELECT 1 FROM catalog.productos v WHERE v.producto_padre_id = p.id)',
    ];
    const params: string[] = [];
    if (empresaId) {
      params.push(empresaId);
      condiciones.unshift('s.empresa_id = $1');
    }
    const filtro = `WHERE ${condiciones.join(' AND ')}`;
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
      `SELECT s.empresa_id, s.sku, s.nombre, s.stock_fisico, s.reservado,
              s.disponible, s.estado, s.precio_base_cents,
              s.valor_disponible_cents, s.valor_total_cents
       FROM catalog.stock_vista s
       JOIN catalog.productos p
         ON p.empresa_id = s.empresa_id AND LOWER(p.sku) = LOWER(s.sku)
       ${filtro}
       ORDER BY s.estado DESC, s.nombre ASC`,
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
