import { Injectable } from '@nestjs/common';
import { DomainError, PgService, Money } from '@core/shared';

export interface LineaInventario {
  empresa_id: string;
  sku: string;
  nombre: string;
  stock: number;           // stock_fisico
  disponible: number;      // catalog.items.stock, ya neto de reservas
  reservado: number;       // reservas activas
  estado: string;
  precio_base: string;
  valor_disponible_cents: number;
  valor_total_cents: number;
}

export interface ProveedorInventario {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  correo: string;
  telefono: string;
  activo: boolean;
}

export interface LoteInventario {
  id: string;
  empresa_id: string;
  item_id: string;
  proveedor_id: string | null;
  codigo_lote: string;
  vence_en: string | null;
  cantidad_inicial: number;
  cantidad_disponible: number;
}

/**
 * H-06 / GET /api/v2/admin/inventario: niveles de stock de la bodega (desde proyección CQRS).
 */
@Injectable()
export class InventarioService {
  constructor(private readonly pg: PgService) {}

  async listar(empresaId?: string): Promise<LineaInventario[]> {
    const condiciones = [
      "i.tipo = 'producto'",
      'i.eliminado_en IS NULL',
      'NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)',
    ];
    const params: string[] = [];
    if (empresaId) {
      params.push(empresaId);
      condiciones.unshift('i.empresa_id = $1');
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
      `WITH reservas AS (
         SELECT COALESCE(item->>'empresa_id', '00000000-0000-0000-0000-000000000000') AS empresa_id,
                LOWER(item->>'sku') AS sku,
                SUM((item->>'cantidad')::int) AS reservado
           FROM catalog.reservas_ordenes ro,
                jsonb_array_elements(ro.items_json) AS item
          WHERE ro.creado_en > NOW() - INTERVAL '30 minutes'
          GROUP BY 1, 2
       )
       SELECT i.empresa_id, i.codigo AS sku, i.nombre,
              i.stock + COALESCE(r.reservado, 0) AS stock_fisico,
              COALESCE(r.reservado, 0)::int AS reservado,
              i.stock AS disponible,
              CASE WHEN i.stock > 0 THEN 'disponible' ELSE 'agotado' END AS estado,
              i.precio_cents AS precio_base_cents,
              (i.stock * i.precio_cents)::int AS valor_disponible_cents,
              ((i.stock + COALESCE(r.reservado, 0)) * i.precio_cents)::int AS valor_total_cents
         FROM catalog.items i
         LEFT JOIN reservas r
           ON r.empresa_id = i.empresa_id::text AND r.sku = LOWER(i.codigo)
       ${filtro}
       ORDER BY estado DESC, i.nombre ASC`,
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
      valor_disponible_cents: f.valor_disponible_cents,
      valor_total_cents: f.valor_total_cents,
    }));
  }

  async resumen(empresaId?: string): Promise<{
    productos: number;
    agotados: number;
    stock_total: number;
    valor_inventario_cents: number;
  }> {
    const filtro = empresaId
      ? `WHERE i.empresa_id = $1 AND i.tipo = 'producto' AND i.eliminado_en IS NULL
           AND NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)`
      : `WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL
           AND NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)`;
    const params = empresaId ? [empresaId] : [];
    return this.pg
      .queryOne<{
        productos: number;
        agotados: number;
        stock_total: number;
        valor_inventario_cents: number;
      }>(
        `SELECT COUNT(*)::int AS productos,
                COUNT(*) FILTER (WHERE i.stock = 0)::int AS agotados,
                COALESCE(SUM(i.stock), 0)::int AS stock_total,
                COALESCE(SUM(i.stock * i.precio_cents), 0)::int AS valor_inventario_cents
           FROM catalog.items i ${filtro}`,
        params,
      )
      .then(
        (r) => r ?? { productos: 0, agotados: 0, stock_total: 0, valor_inventario_cents: 0 },
      );
  }

  async crearProveedor(datos: {
    empresa_id: string;
    codigo: string;
    nombre: string;
    correo?: string;
    telefono?: string;
  }): Promise<ProveedorInventario> {
    const fila = await this.pg.queryOne<ProveedorInventario>(
      `INSERT INTO catalog.proveedores (empresa_id, codigo, nombre, correo, telefono)
       VALUES ($1, UPPER($2), $3, $4, $5)
       RETURNING id, empresa_id, codigo, nombre, correo, telefono, activo`,
      [datos.empresa_id, datos.codigo.trim(), datos.nombre.trim(), datos.correo?.trim() ?? '', datos.telefono?.trim() ?? ''],
    );
    if (!fila) throw new DomainError('PROVEEDOR_NO_CREADO', 'No se pudo crear el proveedor.');
    return fila;
  }

  async crearLote(datos: {
    empresa_id: string;
    item_id: string;
    proveedor_id?: string;
    codigo_lote: string;
    vence_en?: string;
    cantidad_inicial: number;
  }): Promise<LoteInventario> {
    const item = await this.pg.queryOne<{ track_inventory: boolean }>(
      `SELECT track_inventory FROM catalog.items WHERE id = $1 AND empresa_id = $2 AND estado = 'activo'`,
      [datos.item_id, datos.empresa_id],
    );
    if (!item) throw new DomainError('ITEM_NO_ENCONTRADO', 'El item no existe para esta empresa.');
    if (!item.track_inventory) throw new DomainError('INVENTARIO_NO_APLICA', 'Los lotes solo aplican a items con inventario.');
    if (!Number.isInteger(datos.cantidad_inicial) || datos.cantidad_inicial < 0) {
      throw new DomainError('CANTIDAD_INVALIDA', 'La cantidad inicial debe ser un entero mayor o igual a cero.');
    }
    const fila = await this.pg.queryOne<LoteInventario>(
      `INSERT INTO catalog.lotes_inventario
         (empresa_id, item_id, proveedor_id, codigo_lote, vence_en, cantidad_inicial, cantidad_disponible)
       VALUES ($1, $2, $3, $4, $5, $6, $6)
       RETURNING id, empresa_id, item_id, proveedor_id, codigo_lote, vence_en,
                 cantidad_inicial, cantidad_disponible`,
      [datos.empresa_id, datos.item_id, datos.proveedor_id ?? null, datos.codigo_lote.trim(), datos.vence_en ?? null, datos.cantidad_inicial],
    );
    if (!fila) throw new DomainError('LOTE_NO_CREADO', 'No se pudo crear el lote.');
    return fila;
  }

  async listarLotes(empresaId: string, itemId?: string): Promise<LoteInventario[]> {
    const params: unknown[] = [empresaId];
    const condiciones = ['empresa_id = $1'];
    if (itemId) {
      params.push(itemId);
      condiciones.push(`item_id = $${params.length}`);
    }
    return this.pg.query<LoteInventario>(
      `SELECT id, empresa_id, item_id, proveedor_id, codigo_lote, vence_en,
              cantidad_inicial, cantidad_disponible
         FROM catalog.lotes_inventario
        WHERE ${condiciones.join(' AND ')}
        ORDER BY vence_en NULLS LAST, codigo_lote`,
      params,
    );
  }
}
