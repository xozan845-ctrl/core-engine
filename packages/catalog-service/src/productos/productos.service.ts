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
  EMPRESA_LEGACY_ID,
} from '@core/shared';

export interface Producto {
  id: string;
  empresa_id: string;
  sku: string;
  nombre: string;
  descripcion: string;
  categoria: string;
  precio_base: string;
  stock: number;
  estado: 'disponible' | 'agotado';
  creado_en: string;
}

/** Clave compuesta de un producto en el catálogo por empresa (ADR-27). */
export interface ClaveProducto {
  empresa_id: string;
  sku: string;
}

interface ProductoRow {
  id: string;
  empresa_id: string;
  sku: string;
  nombre: string;
  descripcion: string;
  categoria: string;
  precio_base_cents: number;
  stock: number;
  estado: string;
  creado_en: string;
}

interface EmpresaCatalogo {
  estado: string;
  tipo_empresa?: string;
}

/**
 * Catálogo maestro **por empresa** (ADR-27) con control de stock y esquema
 * lógico propio (Tabla 7). La escritura es CRUD convencional (baja tasa de
 * escritura, sin sobre-ingeniería según el capítulo 3.2).
 */
@Injectable()
export class ProductosService {
  private readonly identidadUrl = process.env.IDENTITY_SERVICE_URL ?? 'http://identity-service:3001';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';

  constructor(private readonly pg: PgService) {}

  async listar(filtros: {
    empresa_id?: unknown;
    pagina?: unknown;
    limite?: unknown;
    q?: unknown;
    estado?: unknown;
    categoria?: unknown;
  }): Promise<Pagina<Producto>> {
    const empresaId = this.exigirEmpresa(filtros.empresa_id);
    const { pagina, limite } = parsearPaginacion(filtros as Record<string, unknown>);
    const params: unknown[] = [empresaId];
    const condiciones: string[] = ['empresa_id = $1'];
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
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(`SELECT COUNT(*)::int AS n FROM catalog.producto_catalogo_vista ${where}`, params)
      .then((r) => r?.n ?? 0);
    const offset = (pagina - 1) * limite;
    const filas = await this.pg.query<ProductoRow>(
      `SELECT id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents, disponible AS stock, estado, creado_en
       FROM catalog.producto_catalogo_vista ${where} ORDER BY creado_en DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, offset],
    );
    return crearPagina(filas.map((f) => this.serializar(f)), total, { pagina, limite });
  }

  async encontrarPorId(id: string, empresaId?: string): Promise<Producto | null> {
    if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) {
      return null;
    }
    const params: unknown[] = [id];
    let where = 'id = $1';
    if (empresaId) {
      params.push(empresaId);
      where += ` AND empresa_id = $${params.length}`;
    }
    const fila = await this.pg.queryOne<ProductoRow>(
      `SELECT id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents, disponible AS stock, estado, creado_en
       FROM catalog.producto_catalogo_vista WHERE ${where}`,
      params,
    );
    return fila ? this.serializar(fila) : null;
  }

  async encontrarPorSku(sku: string, empresaId?: string): Promise<Producto | null> {
    const params: unknown[] = [sku];
    let where = 'LOWER(sku) = LOWER($1)';
    if (empresaId) {
      params.push(empresaId);
      where += ` AND empresa_id = $${params.length}`;
    }
    const fila = await this.pg.queryOne<ProductoRow>(
      `SELECT id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents, disponible AS stock, estado, creado_en
       FROM catalog.producto_catalogo_vista WHERE ${where}`,
      params,
    );
    return fila ? this.serializar(fila) : null;
  }

  /**
   * Consulta por lote de SKUs. Un SKU puede repetirse en empresas distintas,
   * por eso la fila incluye `empresa_id`: el consumidor debe emparejar la
   * clave compuesta (ADR-27).
   */
  async lotePorSkus(skus: string[]): Promise<{ empresa_id: string; sku: string; stock: number; estado: string }[]> {
    if (!skus.length) return [];
    const params: string[] = [...new Set(skus.map((s) => String(s).toLowerCase()))];
    const marcadores = params.map((_, i) => `$${i + 1}`).join(', ');
    const rows = await this.pg.query<{ empresa_id: string; sku: string; stock: number; estado: string }>(
      `SELECT empresa_id, LOWER(sku) AS sku, disponible AS stock, estado
       FROM catalog.producto_catalogo_vista
       WHERE LOWER(sku) IN (${marcadores})
       ORDER BY creado_en DESC`,
      params,
    );
    return rows;
  }

  /** Stock restante por producto (alimenta stock.updated para RN-02). */
  async stockActualDe(items: ClaveProducto[]): Promise<{ empresa_id: string; sku: string; stock_restante: number }[]> {
    if (!items.length) return [];
    const claves = [...new Map(items.map((i) => [`${i.empresa_id}:${i.sku.toLowerCase()}`, {
      empresa_id: i.empresa_id,
      sku: String(i.sku).toLowerCase(),
    }])).values()];
    return this.pg.query<{ empresa_id: string; sku: string; stock_restante: number }>(
      `SELECT p.empresa_id, LOWER(p.sku) AS sku, p.stock AS stock_restante
       FROM catalog.productos p
       JOIN jsonb_to_recordset($1::jsonb) AS i(empresa_id uuid, sku text)
         ON p.empresa_id = i.empresa_id AND LOWER(p.sku) = LOWER(i.sku)`,
      [JSON.stringify(claves)],
    );
  }

  /** ADR-27: el catálogo siempre se consulta en el ámbito de una empresa. */
  private exigirEmpresa(valor: unknown): string {
    const empresaId = typeof valor === 'string' ? valor.trim() : '';
    if (!empresaId) {
      throw new DomainError('EMPRESA_REQUERIDA', 'El catálogo se consulta por empresa (empresa_id).');
    }
    if (empresaId === EMPRESA_LEGACY_ID) {
      throw new DomainError('EMPRESA_NO_PUBLICA', 'La empresa comodín del catálogo legado no publica productos.');
    }
    return empresaId;
  }

  async crear(datos: {
    empresa_id: string;
    sku: string;
    nombre: string;
    descripcion?: string;
    categoria?: string;
    precio_base: string | number;
    stock: number;
  }): Promise<Producto> {
    const sku = String(datos.sku).trim().toUpperCase();
    if (!/^[A-Z0-9-]{2,32}$/.test(sku)) {
      throw new DomainError('SKU_INVALIDO', 'El SKU solo admite letras, numeros y guiones (2-32).');
    }
    if (datos.stock < 0 || !Number.isInteger(datos.stock)) {
      throw new DomainError('STOCK_INVALIDO', 'El stock debe ser un entero mayor o igual a 0.');
    }
    const precio = Money.parsear(datos.precio_base);
    if (!precio.esPositivo()) {
      throw new DomainError('PRECIO_INVALIDO', 'El precio base debe ser mayor a 0.');
    }
    const existente = await this.encontrarPorSku(sku, datos.empresa_id);
    if (existente) {
      throw new ConflictError(`Ya existe un producto con el SKU ${sku} en esta empresa.`);
    }
    await this.validarEmpresaCatalogo(datos.empresa_id);
    const fila = await this.pg.transaccion(async (client) => {
      const r = await client.query<ProductoRow>(
        `INSERT INTO catalog.productos (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents, stock, estado)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                 CASE WHEN $8 > 0 THEN 'disponible' ELSE 'agotado' END)
         RETURNING id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents, stock, estado, creado_en`,
        [
          randomUUID(),
          datos.empresa_id,
          sku,
          datos.nombre.trim(),
          (datos.descripcion ?? '').trim(),
          (datos.categoria ?? 'general').trim(),
          precio.centavos,
          datos.stock,
        ],
      );
      await client.query(
        `INSERT INTO catalog.historico_precios (id, empresa_id, producto_sku, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
         VALUES (gen_random_uuid(), $1, $2, NULL, $3, NOW())`,
        [datos.empresa_id, sku, precio.centavos],
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
    empresaId?: string,
  ): Promise<Producto> {
    const previo = await this.encontrarPorId(id, empresaId);
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
          `INSERT INTO catalog.historico_precios (empresa_id, producto_sku, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
           VALUES ($1, $2, $3, $4, NOW())`,
          [previo.empresa_id, previo.sku, Money.parsear(previo.precio_base).centavos, nuevo.centavos],
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
          `INSERT INTO catalog.ajustes_stock (id, empresa_id, producto_sku, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
           VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW())`,
          [previo.empresa_id, previo.sku, previo.stock, cambios.stock, cambios.motivo ?? 'ajuste manual'],
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
    const actualizado = await this.encontrarPorId(id, empresaId);
    if (!actualizado) throw new NotFoundError('Producto', id);
    return actualizado;
  }

  /**
   * ADR-27: solo una empresa activa de tipo comercial tiene catálogo. Se
   * consulta a identity por API interna (R-DB-1) y se falla cerrado si no se
   * puede verificar.
   */
  private async validarEmpresaCatalogo(empresaId: string): Promise<void> {
    if (!empresaId || empresaId === EMPRESA_LEGACY_ID) {
      throw new DomainError('EMPRESA_NO_PUBLICA', 'La empresa comodín del catálogo legado no publica productos.');
    }
    let res: Response;
    try {
      res = await fetch(
        `${this.identidadUrl}/internal/empresas/${encodeURIComponent(empresaId)}`,
        { headers: { 'x-internal-key': this.claveInterna } },
      );
    } catch {
      throw new DomainError('EMPRESA_NO_VERIFICABLE', 'No se pudo verificar la empresa del catálogo.');
    }
    if (res.status === 404) throw new NotFoundError('Empresa', empresaId);
    if (!res.ok) {
      throw new DomainError('EMPRESA_NO_VERIFICABLE', 'No se pudo verificar la empresa del catálogo.');
    }
    const empresa = (await res.json()) as EmpresaCatalogo;
    if (empresa.estado !== 'activa') {
      throw new DomainError('EMPRESA_INACTIVA', 'La empresa debe estar activa para operar su catálogo.');
    }
    if (empresa.tipo_empresa && empresa.tipo_empresa !== 'comercial') {
      throw new DomainError('EMPRESA_SIN_CATALOGO', 'Solo las empresas comerciales tienen catálogo propio.');
    }
  }

  /**
   * RN-03: descuento atomico de stock dentro de una transaccion.
   * Si alguna linea no alcanza stock, se rechaza la reserva completa (la
   * transaccion revierte las lineas ya descontadas).
   * La clave es (empresa, SKU): cada empresa tiene su propio stock (ADR-27).
   */
  async reservarStock(
    orderId: string,
    items: { empresa_id: string; sku: string; cantidad: number }[],
  ): Promise<{
    ok: boolean;
    fallidos: { empresa_id: string; sku: string; cantidad: number; motivo: string }[];
    stock_restante?: { empresa_id: string; sku: string; stock_restante: number }[];
  }> {
    const fallidos: { empresa_id: string; sku: string; cantidad: number; motivo: string }[] = [];
    let reservado = false;
    try {
      await this.pg.transaccion(async (client) => {
        for (const item of items) {
          // Bloqueo pesimista requerido por regla dorada (Data Integrity)
          const lock = await client.query(
            `SELECT stock FROM catalog.productos
             WHERE empresa_id = $1 AND LOWER(sku) = LOWER($2) FOR UPDATE`,
            [item.empresa_id, item.sku]
          );

          if (lock.rowCount === 0 || lock.rows[0].stock < item.cantidad) {
            fallidos.push({
              empresa_id: item.empresa_id,
              sku: item.sku,
              cantidad: item.cantidad,
              motivo: 'stock_insuficiente',
            });
            continue;
          }

          await client.query(
            `UPDATE catalog.productos
             SET stock = stock - $3,
                 estado = CASE WHEN stock - $3 > 0 THEN 'disponible' ELSE 'agotado' END
             WHERE empresa_id = $1 AND LOWER(sku) = LOWER($2)
             RETURNING sku`,
            [item.empresa_id, item.sku, item.cantidad],
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
    const stock = await this.pg.query<{ empresa_id: string; sku: string; stock_restante: number }>(
      `SELECT p.empresa_id, LOWER(p.sku) AS sku, p.stock AS stock_restante
       FROM catalog.productos p
       JOIN jsonb_to_recordset($1::jsonb) AS i(empresa_id uuid, sku text)
         ON p.empresa_id = i.empresa_id AND LOWER(p.sku) = LOWER(i.sku)`,
      [JSON.stringify([...new Map(items.map((i) => [`${i.empresa_id}:${i.sku.toLowerCase()}`, {
        empresa_id: i.empresa_id,
        sku: String(i.sku).toLowerCase(),
      }])).values()])],
    );
    return { ok: true, fallidos, stock_restante: stock };
  }

  /** RN-06: devolucion aprobada revierte stock por (empresa, SKU). */
  async reintegrarStock(
    orderId: string,
    items: { empresa_id: string; sku: string; cantidad: number }[],
  ): Promise<void> {
    await this.pg.transaccion(async (client) => {
      for (const item of items) {
        await client.query(
          `UPDATE catalog.productos
           SET stock = stock + $3,
               estado = 'disponible'
           WHERE empresa_id = $1 AND LOWER(sku) = LOWER($2)`,
          [item.empresa_id, item.sku, item.cantidad],
        );
      }
      await client.query(
        `INSERT INTO catalog.ajustes_stock (empresa_id, producto_sku, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
         SELECT p.empresa_id, p.sku, p.stock - i.cantidad, p.stock, 'devolucion orden ' || $1, NOW()
         FROM catalog.productos p
         JOIN jsonb_to_recordset($2::jsonb) AS i(empresa_id uuid, sku text, cantidad int)
           ON p.empresa_id = i.empresa_id AND LOWER(p.sku) = LOWER(i.sku)`,
        [orderId, JSON.stringify(items)],
      );
      // La reserva de esta orden ya no está activa tras reintegrar el stock.
      await client.query(
        `DELETE FROM catalog.reservas_ordenes WHERE order_id = $1`,
        [orderId],
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

  resumenInventario(empresaId?: string): Promise<{
    productos: number;
    agotados: number;
    stock_total: number;
    valor_inventario_cents: number;
  }> {
    const filtro = empresaId ? 'WHERE empresa_id = $1' : '';
    const params = empresaId ? [empresaId] : [];
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
         FROM catalog.productos ${filtro}`,
        params,
      )
      .then((r) =>
        r ?? { productos: 0, agotados: 0, stock_total: 0, valor_inventario_cents: 0 },
      );
  }

  private serializar(fila: ProductoRow): Producto {
    return {
      id: fila.id,
      empresa_id: fila.empresa_id,
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
