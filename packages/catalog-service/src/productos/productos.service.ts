import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  PgService,
  DomainError,
  ConflictError,
  NotFoundError,
  Money,
  crearPagina,
  parsearPaginacion,
  Pagina,
} from '@core/shared';

export interface Producto {
  id: string;
  sku: string;
  nombre: string;
  descripcion: string;
  categoria: string;
  precio_base: string;
  stock: number;
  estado: 'disponible' | 'agotado';
  creado_en: string;
}

interface ProductoRow {
  id: string;
  sku: string;
  nombre: string;
  descripcion: string;
  categoria: string;
  precio_base_cents: number;
  stock: number;
  estado: string;
  creado_en: string;
}

/**
 * Catalogo maestro y control de stock con esquema logico propio (Tabla 7).
 * La escritura es CRUD convencional (baja tasa de escritura, sin sobre-ingenieria
 * segun el capitulo 3.2).
 */
@Injectable()
export class ProductosService {
  constructor(private readonly pg: PgService) {}

  async listar(filtros: {
    pagina?: unknown;
    limite?: unknown;
    q?: unknown;
    estado?: unknown;
    categoria?: unknown;
  }): Promise<Pagina<Producto>> {
    const { pagina, limite } = parsearPaginacion(filtros as Record<string, unknown>);
    const condiciones: string[] = [];
    const params: unknown[] = [];
    if (filtros.q) {
      params.push(`%${String(filtros.q).toLowerCase()}%`);
      condiciones.push(`(LOWER(nombre) LIKE $${params.length} OR LOWER(sku) LIKE $${params.length})`);
    }
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`estado = $${params.length}`);
    }
    if (filtros.categoria) {
      params.push(String(filtros.categoria));
      condiciones.push(`LOWER(categoria) = LOWER($${params.length})`);
    }
    const where = condiciones.length ? `WHERE ${condiciones.join(' AND ')}` : '';
    const total = await this.pg
      .queryOne<{ n: number }>(`SELECT COUNT(*)::int AS n FROM catalog.producto_catalogo_vista ${where}`, params)
      .then((r) => r?.n ?? 0);
    const offset = (pagina - 1) * limite;
    const filas = await this.pg.query<ProductoRow>(
      `SELECT id, sku, nombre, descripcion, categoria, precio_base_cents, disponible AS stock, estado, creado_en
       FROM catalog.producto_catalogo_vista ${where} ORDER BY creado_en DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, offset],
    );
    return crearPagina(filas.map((f) => this.serializar(f)), total, { pagina, limite });
  }

  async encontrarPorId(id: string): Promise<Producto | null> {
    if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) {
      return null;
    }
    const fila = await this.pg.queryOne<ProductoRow>(
      `SELECT id, sku, nombre, descripcion, categoria, precio_base_cents, disponible AS stock, estado, creado_en
       FROM catalog.producto_catalogo_vista WHERE id = $1`,
      [id],
    );
    return fila ? this.serializar(fila) : null;
  }

  async encontrarPorSku(sku: string): Promise<Producto | null> {
    const fila = await this.pg.queryOne<ProductoRow>(
      `SELECT id, sku, nombre, descripcion, categoria, precio_base_cents, disponible AS stock, estado, creado_en
       FROM catalog.producto_catalogo_vista WHERE LOWER(sku) = LOWER($1)`,
      [sku],
    );
    return fila ? this.serializar(fila) : null;
  }

  /** H-03 / TC-01: alta de producto con stock y precio base. */
  /** Consulta por lote de skus (posible SKU duplicado en items: se devuelve una fila por sku). */
  async lotePorSkus(skus: string[]): Promise<{ sku: string; stock: number; estado: string }[]> {
    if (!skus.length) return [];
    const params: string[] = [...new Set(skus.map((s) => String(s).toLowerCase()))];
    const marcadores = params.map((_, i) => `$${i + 1}`).join(', ');
    const rows = await this.pg.query<{ sku: string; stock: number; estado: string }>(
      `SELECT sku, disponible AS stock, estado
       FROM catalog.producto_catalogo_vista
       WHERE LOWER(sku) IN (${marcadores})
       ORDER BY creado_en DESC`,
      params,
    );
    return rows;
  }

  /** Stock restante por SKU (alimenta stock.updated para RN-02). */
  async stockActualDe(skus: string[]): Promise<{ sku: string; stock_restante: number }[]> {
    if (!skus.length) return [];
    const params: string[] = [...new Set(skus.map((s) => String(s).toLowerCase()))];
    const marcadores = params.map((_, i) => `$${i + 1}`).join(', ');
    return this.pg.query<{ sku: string; stock_restante: number }>(
      `SELECT LOWER(sku) AS sku, disponible AS stock_restante
       FROM catalog.producto_catalogo_vista
       WHERE LOWER(sku) IN (${marcadores})`,
      params,
    );
  }

  async crear(datos: {
    sku: string;
    nombre: string;
    descripcion?: string;
    categoria?: string;
    precio_base: string | number;
    stock: number;
  }): Promise<Producto> {
    const sku = String(datos.sku).trim().toUpperCase();
    if (!/^[A-Z0-9\-]{2,32}$/.test(sku)) {
      throw new DomainError('SKU_INVALIDO', 'El SKU solo admite letras, numeros y guiones (2-32).');
    }
    if (datos.stock < 0 || !Number.isInteger(datos.stock)) {
      throw new DomainError('STOCK_INVALIDO', 'El stock debe ser un entero mayor o igual a 0.');
    }
    const precio = Money.parsear(datos.precio_base);
    if (!precio.esPositivo()) {
      throw new DomainError('PRECIO_INVALIDO', 'El precio base debe ser mayor a 0.');
    }
    const existente = await this.encontrarPorSku(sku);
    if (existente) {
      throw new ConflictError(`Ya existe un producto con el SKU ${sku}.`);
    }
    const fila = await this.pg.transaccion(async (client) => {
      const r = await client.query<ProductoRow>(
        `INSERT INTO catalog.productos (id, sku, nombre, descripcion, categoria, precio_base_cents, stock, estado)
         VALUES ($1, $2, $3, $4, $5, $6, $7,
                 CASE WHEN $7 > 0 THEN 'disponible' ELSE 'agotado' END)
         RETURNING id, sku, nombre, descripcion, categoria, precio_base_cents, stock, estado, creado_en`,
        [
          randomUUID(),
          sku,
          datos.nombre.trim(),
          (datos.descripcion ?? '').trim(),
          (datos.categoria ?? 'general').trim(),
          precio.centavos,
          datos.stock,
        ],
      );
      await client.query(
        `INSERT INTO catalog.historico_precios (id, producto_sku, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
         VALUES (gen_random_uuid(), $1, NULL, $2, NOW())`,
        [sku, precio.centavos],
      );
      return r.rows[0];
    });
    return this.serializar(fila);
  }

  /**
   * H-03: edicion del catalogo. Cambios de precio base van al historico (RN-08:
   * aplica a nuevas ofertas, nunca a las ya activas). Ajustes de stock se auditan.
   */
  async actualizar(
    id: string,
    cambios: {
      nombre?: string;
      descripcion?: string;
      categoria?: string;
      precio_base?: string | number;
      stock?: number;
      motivo?: string;
    },
  ): Promise<Producto> {
    const previo = await this.encontrarPorId(id);
    if (!previo) throw new NotFoundError('Producto', id);

    await this.pg.transaccion(async (client) => {
      const sets: string[] = [];
      const params: unknown[] = [];
      const push = (sql: string, valor: unknown): void => {
        params.push(valor);
        sets.push(sql.replace('$', `$${params.length}`));
      };
      if (cambios.nombre !== undefined) push('nombre = $', cambios.nombre.trim());
      if (cambios.descripcion !== undefined) push('descripcion = $', cambios.descripcion.trim());
      if (cambios.categoria !== undefined) push('categoria = $', cambios.categoria.trim());

      if (cambios.precio_base !== undefined) {
        const nuevo = Money.parsear(cambios.precio_base);
        if (!nuevo.esPositivo()) {
          throw new DomainError('PRECIO_INVALIDO', 'El precio base debe ser mayor a 0.');
        }
        push('precio_base_cents = $', nuevo.centavos);
        // RN-08: trazabilidad de cambios del precio base
        await client.query(
          `INSERT INTO catalog.historico_precios (producto_sku, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
           VALUES ($1, $2, $3, NOW())`,
          [previo.sku, Money.parsear(previo.precio_base).centavos, nuevo.centavos],
        );
      }

      if (cambios.stock !== undefined) {
        if (cambios.stock < 0 || !Number.isInteger(cambios.stock)) {
          throw new DomainError('STOCK_INVALIDO', 'El stock debe ser un entero mayor o igual a 0.');
        }
        push('stock = $', cambios.stock);
        push('estado = $', cambios.stock > 0 ? 'disponible' : 'agotado');
        // auditoria de movimientos manuales (mermas, correcciones)
        await client.query(
          `INSERT INTO catalog.ajustes_stock (id, producto_sku, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
           VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW())`,
          [previo.sku, previo.stock, cambios.stock, cambios.motivo ?? 'ajuste manual'],
        );
      }

      params.push(id);
      if (sets.length > 0) {
        await client.query(
          `UPDATE catalog.productos SET ${sets.join(', ')} WHERE id = $${params.length}`,
          params,
        );
      }
    });
    const actualizado = await this.encontrarPorId(id);
    if (!actualizado) throw new NotFoundError('Producto', id);
    return actualizado;
  }

  /**
   * RN-03: descuento atomico de stock dentro de una transaccion.
   * Si alguna linea no alcanza stock, se rechaza la reserva completa (la
   * transaccion revierte las lineas ya descontadas).
   * Devuelve el stock restante por SKU (alimenta stock.updated para RN-02).
   */
  async reservarStock(
    orderId: string,
    items: { sku: string; cantidad: number }[],
  ): Promise<{
    ok: boolean;
    fallidos: { sku: string; cantidad: number; motivo: string }[];
    stock_restante?: { sku: string; stock_restante: number }[];
  }> {
    const fallidos: { sku: string; cantidad: number; motivo: string }[] = [];
    let reservado = false;
    try {
      await this.pg.transaccion(async (client) => {
        for (const item of items) {
          // Bloqueo pesimista requerido por regla dorada (Data Integrity)
          const lock = await client.query(
            `SELECT stock FROM catalog.productos WHERE LOWER(sku) = LOWER($1) FOR UPDATE`,
            [item.sku]
          );
          
          if (lock.rowCount === 0 || lock.rows[0].stock < item.cantidad) {
            fallidos.push({ sku: item.sku, cantidad: item.cantidad, motivo: 'stock_insuficiente' });
            continue;
          }

          const actualizado = await client.query(
            `UPDATE catalog.productos
             SET stock = stock - $2,
                 estado = CASE WHEN stock - $2 > 0 THEN 'disponible' ELSE 'agotado' END
             WHERE LOWER(sku) = LOWER($1)
             RETURNING sku`,
            [item.sku, item.cantidad],
          );
        }
        if (fallidos.length > 0) {
          throw new Error('STOCK_INSUFICIENTE');
        }
        await client.query(
          `INSERT INTO catalog.reservas_ordenes (order_id, items_json, creado_en)
           VALUES ($1, $2, NOW())`,
          [orderId, JSON.stringify(items)],
        );
        reservado = true;
      });
    } catch (err) {
      if (!(err instanceof Error) || err.message !== 'STOCK_INSUFICIENTE') throw err;
    }
    if (!reservado) return { ok: false, fallidos };
    const stock = await this.pg.query<{ sku: string; stock_restante: number }>(
      `SELECT LOWER(sku) AS sku, stock AS stock_restante
       FROM catalog.productos
       WHERE LOWER(sku) = ANY($1)`,
      [[...new Set(items.map((i) => String(i.sku).toLowerCase()))]],
    );
    return { ok: true, fallidos, stock_restante: stock };
  }

  /** RN-06: devolucion aprobada revierte stock. */
  async reintegrarStock(
    orderId: string,
    items: { sku: string; cantidad: number }[],
  ): Promise<void> {
    await this.pg.transaccion(async (client) => {
      for (const item of items) {
        await client.query(
          `UPDATE catalog.productos
           SET stock = stock + $2,
               estado = 'disponible'
           WHERE LOWER(sku) = LOWER($1)`,
          [item.sku, item.cantidad],
        );
      }
      await client.query(
        `INSERT INTO catalog.ajustes_stock (producto_sku, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
         SELECT p.sku, p.stock - i.cantidad, p.stock, 'devolucion orden ' || $1, NOW()
         FROM catalog.productos p, jsonb_to_recordset($2::jsonb) AS i(sku text, cantidad int)
         WHERE p.sku = i.sku`,
        [orderId, JSON.stringify(items)],
      );
    });
  }

  /** Claves de eventos ya procesadas (idempotencia, doc 5.2). */
  async estaProcesado(event_id: string): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM catalog.eventos_procesados WHERE event_id = $1`,
      [event_id],
    );
    return !!fila;
  }

  async registrarProcesado(event_id: string, tipo: string): Promise<void> {
    await this.pg.query(
      `INSERT INTO catalog.eventos_procesados (event_id, tipo, procesado_en)
       VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING`,
      [event_id, tipo],
    );
  }

  resumenInventario(): Promise<{
    productos: number;
    agotados: number;
    stock_total: number;
    valor_inventario_cents: number;
  }> {
    return this.pg
      .queryOne<{
        productos: number;
        agotados: number;
        stock_total: number;
        valor_inventario_cents: number;
      }>(
        `SELECT COUNT(*)::int AS productos,
                COUNT(*) FILTER (WHERE estado = 'agotado')::int AS agotados,
                COALESCE(SUM(stock), 0)::int AS stock_total,
                COALESCE(SUM(stock * precio_base_cents), 0)::int AS valor_inventario_cents
         FROM catalog.productos`,
      )
      .then((r) =>
        r ?? { productos: 0, agotados: 0, stock_total: 0, valor_inventario_cents: 0 },
      );
  }

  private serializar(fila: ProductoRow): Producto {
    return {
      id: fila.id,
      sku: fila.sku,
      nombre: fila.nombre,
      descripcion: fila.descripcion,
      categoria: fila.categoria,
      precio_base: Money.desdeCentavos(fila.precio_base_cents).string(),
      stock: fila.stock,
      estado: fila.estado === 'disponible' ? 'disponible' : 'agotado',
      creado_en: fila.creado_en,
    };
  }
}