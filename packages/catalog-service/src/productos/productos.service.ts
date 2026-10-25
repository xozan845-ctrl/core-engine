import { Injectable, Optional } from '@nestjs/common';
import { randomUUID } from 'crypto';
import type { PoolClient } from 'pg';
import {
  PgService,
  OutboxService,
  DomainError,
  ConflictError,
  NotFoundError,
  Money,
  EVENTOS,
  crearPagina,
  parsearPaginacion,
  Pagina,
  EMPRESA_LEGACY_ID,
} from '@core/shared';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const MAX_IMAGENES = 3;
const MAX_VARIANTES = 50;
const URL_IMAGEN_PUBLICA = '/api/v1/storage/public/objetos';

export interface ImagenProducto {
  id: string;
  objeto_id: string;
  /** Ruta relativa servida por storage-service (ADR-28). */
  url: string;
  orden: number;
  es_principal: boolean;
}

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
  producto_padre_id: string | null;
  atributos: Record<string, unknown>;
  tiene_variantes: boolean;
  imagenes: ImagenProducto[];
  /** Solo en el detalle del padre; en listados queda vacío. */
  variantes?: Producto[];
  /** Precio mínimo mostrable (para "desde"). */
  precio_desde: string;
  /** Stock agregado de las variantes o el propio. */
  stock_total: number;
}

export interface ImagenProductoInput {
  objeto_id: string;
  orden?: number;
  es_principal?: boolean;
}

export interface VarianteProductoInput {
  sku: string;
  nombre?: string;
  atributos?: Record<string, unknown>;
  precio_base: string;
  stock: number;
  imagenes?: ImagenProductoInput[];
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
  producto_padre_id?: string | null;
  atributos?: Record<string, unknown> | null;
  tiene_variantes?: boolean;
}

interface ImagenRow {
  id: string;
  producto_id: string;
  objeto_id: string;
  orden: number;
  es_principal: boolean;
}

interface EmpresaCatalogo {
  estado: string;
  tipo_empresa?: string;
}

/**
 * Catálogo maestro **por empresa** (ADR-27) con variantes e imágenes (ADR-28).
 * Una variante es una fila hija de `catalog.productos`: conserva su SKU, precio
 * y stock, así que las reservas/órdenes siguen operando por (empresa, SKU).
 * El padre agrega stock y precio mínimo y no se vende si tiene variantes.
 */
@Injectable()
export class ProductosService {
  private readonly identidadUrl = process.env.IDENTITY_SERVICE_URL ?? 'http://identity-service:3001';
  private readonly storageUrl = process.env.STORAGE_SERVICE_URL ?? 'http://storage-service:3010';
  private readonly storesUrl = process.env.STORES_SERVICE_URL ?? 'http://stores-service:3003';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';

  constructor(
    private readonly pg: PgService,
    @Optional() private readonly outbox?: OutboxService,
  ) {}

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
    const condiciones: string[] = ['v.empresa_id = $1', 'v.producto_padre_id IS NULL'];
    if (filtros.q) {
      params.push(`%${String(filtros.q).toLowerCase()}%`);
      condiciones.push(
        `(LOWER(v.nombre) LIKE $${params.length} OR LOWER(v.sku) LIKE $${params.length})`,
      );
    }
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`v.estado = $${params.length}`);
    }
    if (filtros.categoria) {
      params.push(String(filtros.categoria));
      condiciones.push(`LOWER(v.categoria) = LOWER($${params.length})`);
    }
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(
        `SELECT COUNT(*)::int AS n FROM catalog.producto_catalogo_vista v ${where}`,
        params,
      )
      .then((r) => r?.n ?? 0);
    const offset = (pagina - 1) * limite;
    const filas = await this.pg.query<ProductoRow>(
      `SELECT v.id, v.empresa_id, v.sku, v.nombre, v.descripcion, v.categoria,
              v.precio_base_cents, v.disponible AS stock, v.estado, v.creado_en,
              v.producto_padre_id, v.atributos,
              EXISTS (SELECT 1 FROM catalog.productos c WHERE c.producto_padre_id = v.id) AS tiene_variantes
       FROM catalog.producto_catalogo_vista v ${where}
       ORDER BY v.creado_en DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, offset],
    );
    const imagenes = await this.imagenesDe(filas.map((f) => f.id));
    return crearPagina(
      filas.map((f) => this.serializar(f, imagenes.get(f.id) ?? [])),
      total,
      { pagina, limite },
    );
  }

  async encontrarPorId(id: string, empresaId?: string): Promise<Producto | null> {
    if (!UUID.test(id)) return null;
    const params: unknown[] = [id];
    let where = 'v.id = $1';
    if (empresaId) {
      params.push(empresaId);
      where += ` AND v.empresa_id = $${params.length}`;
    }
    const fila = await this.pg.queryOne<ProductoRow>(
      `SELECT v.id, v.empresa_id, v.sku, v.nombre, v.descripcion, v.categoria,
              v.precio_base_cents, v.disponible AS stock, v.estado, v.creado_en,
              v.producto_padre_id, v.atributos,
              EXISTS (SELECT 1 FROM catalog.productos c WHERE c.producto_padre_id = v.id) AS tiene_variantes
       FROM catalog.producto_catalogo_vista v WHERE ${where}`,
      params,
    );
    return fila ? this.detallar(fila) : null;
  }

  async encontrarPorSku(sku: string, empresaId?: string): Promise<Producto | null> {
    const params: unknown[] = [sku];
    let where = 'LOWER(v.sku) = LOWER($1)';
    if (empresaId) {
      params.push(empresaId);
      where += ` AND v.empresa_id = $${params.length}`;
    }
    const fila = await this.pg.queryOne<ProductoRow>(
      `SELECT v.id, v.empresa_id, v.sku, v.nombre, v.descripcion, v.categoria,
              v.precio_base_cents, v.disponible AS stock, v.estado, v.creado_en,
              v.producto_padre_id, v.atributos,
              EXISTS (SELECT 1 FROM catalog.productos c WHERE c.producto_padre_id = v.id) AS tiene_variantes
       FROM catalog.producto_catalogo_vista v WHERE ${where}`,
      params,
    );
    return fila ? this.detallar(fila) : null;
  }

  /**
   * Consulta por lote de SKUs. Un SKU puede repetirse en empresas distintas,
   * por eso la fila incluye `empresa_id`: el consumidor debe emparejar la
   * clave compuesta (ADR-27).
   */
  async lotePorSkus(
    skus: string[],
  ): Promise<{ empresa_id: string; sku: string; stock: number; estado: string }[]> {
    if (!skus.length) return [];
    const params: string[] = [...new Set(skus.map((s) => String(s).toLowerCase()))];
    const marcadores = params.map((_, i) => `$${i + 1}`).join(', ');
    return this.pg.query<{ empresa_id: string; sku: string; stock: number; estado: string }>(
      `SELECT empresa_id, LOWER(sku) AS sku, disponible AS stock, estado
       FROM catalog.producto_catalogo_vista
       WHERE LOWER(sku) IN (${marcadores})
       ORDER BY creado_en DESC`,
      params,
    );
  }

  /** Stock restante por producto (alimenta stock.updated para RN-02). */
  async stockActualDe(
    items: ClaveProducto[],
  ): Promise<{ empresa_id: string; sku: string; stock_restante: number }[]> {
    if (!items.length) return [];
    const claves = [
      ...new Map(
        items.map((i) => [
          `${i.empresa_id}:${i.sku.toLowerCase()}`,
          {
            empresa_id: i.empresa_id,
            sku: String(i.sku).toLowerCase(),
          },
        ]),
      ).values(),
    ];
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
      throw new DomainError(
        'EMPRESA_NO_PUBLICA',
        'La empresa comodín del catálogo legado no publica productos.',
      );
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
    atributos?: Record<string, unknown>;
    variantes?: VarianteProductoInput[];
    imagenes?: ImagenProductoInput[];
  }): Promise<Producto> {
    const sku = this.normalizarSku(datos.sku);
    this.validarStock(datos.stock);
    const precio = this.validarPrecio(datos.precio_base);
    const variantes = datos.variantes ?? [];
    if (variantes.length > MAX_VARIANTES) {
      throw new DomainError(
        'DEMASIADAS_VARIANTES',
        `Un producto admite como máximo ${MAX_VARIANTES} variantes.`,
      );
    }
    const skus = new Set<string>([sku]);
    for (const variante of variantes) {
      const varianteSku = this.normalizarSku(variante.sku);
      this.validarStock(variante.stock);
      this.validarPrecio(variante.precio_base);
      if (skus.has(varianteSku)) {
        throw new ConflictError(`El SKU ${varianteSku} está repetido en la solicitud.`);
      }
      skus.add(varianteSku);
    }
    if (await this.skuExiste(datos.empresa_id, sku)) {
      throw new ConflictError(`Ya existe un producto con el SKU ${sku} en esta empresa.`);
    }
    for (const variante of variantes) {
      if (await this.skuExiste(datos.empresa_id, this.normalizarSku(variante.sku))) {
        throw new ConflictError(
          `Ya existe un producto con el SKU ${this.normalizarSku(variante.sku)} en esta empresa.`,
        );
      }
    }
    await this.validarEmpresaCatalogo(datos.empresa_id);
    await this.validarImagenes(datos.empresa_id, datos.imagenes ?? []);
    for (const variante of variantes) {
      await this.validarImagenes(datos.empresa_id, variante.imagenes ?? []);
    }

    const padreId = randomUUID();
    await this.pg.transaccion(async (client) => {
      await client.query(
        `INSERT INTO catalog.productos
           (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents,
            stock, estado, producto_padre_id, atributos)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                 CASE WHEN $8 > 0 THEN 'disponible' ELSE 'agotado' END, NULL, $9::jsonb)`,
        [
          padreId,
          datos.empresa_id,
          sku,
          datos.nombre.trim(),
          (datos.descripcion ?? '').trim(),
          (datos.categoria ?? 'general').trim(),
          precio.centavos,
          datos.stock,
          JSON.stringify(datos.atributos ?? {}),
        ],
      );
      await this.registrarHistorico(client, datos.empresa_id, sku, null, precio.centavos);

      for (const variante of variantes) {
        const varianteId = randomUUID();
        const varianteSku = this.normalizarSku(variante.sku);
        const variantePrecio = this.validarPrecio(variante.precio_base);
        await client.query(
          `INSERT INTO catalog.productos
             (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents,
              stock, estado, producto_padre_id, atributos)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                   CASE WHEN $8 > 0 THEN 'disponible' ELSE 'agotado' END, $9, $10::jsonb)`,
          [
            varianteId,
            datos.empresa_id,
            varianteSku,
            (variante.nombre ?? datos.nombre).trim(),
            (datos.descripcion ?? '').trim(),
            (datos.categoria ?? 'general').trim(),
            variantePrecio.centavos,
            variante.stock,
            padreId,
            JSON.stringify(variante.atributos ?? {}),
          ],
        );
        await this.registrarHistorico(
          client,
          datos.empresa_id,
          varianteSku,
          null,
          variantePrecio.centavos,
        );
        await this.insertarImagenes(
          client,
          datos.empresa_id,
          varianteId,
          variante.imagenes ?? [],
        );
      }

      await this.insertarImagenes(client, datos.empresa_id, padreId, datos.imagenes ?? []);
      if (variantes.length > 0) {
        await client.query('SELECT catalog.fn_recalcular_padre($1)', [padreId]);
      }
    });

    const creado = await this.encontrarPorId(padreId, datos.empresa_id);
    if (!creado) throw new NotFoundError('Producto', padreId);
    return creado;
  }

  /** Agrega una variante a un producto padre (ADR-28). */
  async crearVariante(
    padreId: string,
    datos: VarianteProductoInput,
    empresaId: string,
  ): Promise<Producto> {
    const padre = await this.encontrarPorId(padreId, empresaId);
    if (!padre) throw new NotFoundError('Producto', padreId);
    if (padre.producto_padre_id) {
      throw new DomainError('NO_ES_PADRE', 'Solo un producto padre puede recibir variantes.');
    }
    if ((padre.variantes?.length ?? 0) >= MAX_VARIANTES) {
      throw new DomainError(
        'DEMASIADAS_VARIANTES',
        `Un producto admite como máximo ${MAX_VARIANTES} variantes.`,
      );
    }
    await this.validarSinOfertasActivas(padreId);
    const sku = this.normalizarSku(datos.sku);
    this.validarStock(datos.stock);
    const precio = this.validarPrecio(datos.precio_base);
    if (await this.skuExiste(empresaId, sku)) {
      throw new ConflictError(`Ya existe un producto con el SKU ${sku} en esta empresa.`);
    }
    await this.validarImagenes(empresaId, datos.imagenes ?? []);

    const varianteId = randomUUID();
    await this.pg.transaccion(async (client) => {
      await client.query(
        `INSERT INTO catalog.productos
           (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents,
            stock, estado, producto_padre_id, atributos)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                 CASE WHEN $8 > 0 THEN 'disponible' ELSE 'agotado' END, $9, $10::jsonb)`,
        [
          varianteId,
          empresaId,
          sku,
          (datos.nombre ?? padre.nombre).trim(),
          padre.descripcion,
          padre.categoria,
          precio.centavos,
          datos.stock,
          padreId,
          JSON.stringify(datos.atributos ?? {}),
        ],
      );
      await this.registrarHistorico(client, empresaId, sku, null, precio.centavos);
      await this.insertarImagenes(client, empresaId, varianteId, datos.imagenes ?? []);
      await client.query('SELECT catalog.fn_recalcular_padre($1)', [padreId]);
    });

    const creada = await this.encontrarPorId(varianteId, empresaId);
    if (!creada) throw new NotFoundError('Variante', varianteId);
    return creada;
  }

  /**
   * H-03: edición del catálogo. Cambios de precio base van al histórico (RN-08:
   * aplica a nuevas ofertas, nunca a las ya activas). Ajustes de stock se auditan.
   * Si el producto tiene variantes, el precio/stock se edita en cada variante.
   */
  async actualizar(
    id: string,
    cambios: {
      nombre?: string;
      descripcion?: string;
      categoria?: string;
      precio_base?: string | number;
      stock?: number;
      atributos?: Record<string, unknown>;
      motivo?: string;
    },
    empresaId?: string,
  ): Promise<Producto> {
    const previo = await this.encontrarPorId(id, empresaId);
    if (!previo) throw new NotFoundError('Producto', id);
    if (previo.tiene_variantes && (cambios.precio_base !== undefined || cambios.stock !== undefined)) {
      throw new DomainError(
        'PRODUCTO_CON_VARIANTES',
        'El precio y el stock de un producto con variantes se editan en cada variante.',
      );
    }

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

      if (cambios.atributos !== undefined) {
        params.push(JSON.stringify(cambios.atributos));
        sets.push(`atributos = $${params.length}::jsonb`);
      }

      if (cambios.precio_base !== undefined) {
        const nuevo = this.validarPrecio(cambios.precio_base);
        push('precio_base_cents = $', nuevo.centavos);
        await this.registrarHistorico(
          client,
          previo.empresa_id,
          previo.sku,
          Money.parsear(previo.precio_base).centavos,
          nuevo.centavos,
        );
      }

      if (cambios.stock !== undefined) {
        this.validarStock(cambios.stock);
        push('stock = $', cambios.stock);
        push('estado = $', cambios.stock > 0 ? 'disponible' : 'agotado');
        await client.query(
          `INSERT INTO catalog.ajustes_stock
             (id, empresa_id, producto_sku, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
           VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW())`,
          [
            previo.empresa_id,
            previo.sku,
            previo.stock,
            cambios.stock,
            cambios.motivo ?? 'ajuste manual',
          ],
        );
        // RN-02: las ofertas vivas deben reflejar el stock ajustado a mano.
        if (this.outbox) {
          await this.outbox.insertarEnTransaccion(client, EVENTOS.STOCK_UPDATED, {
            order_id: `ajuste-${id}`,
            tipo: 'ajuste',
            items: [
              {
                empresa_id: previo.empresa_id,
                sku: previo.sku,
                stock_restante: cambios.stock,
              },
            ],
          });
        }
      }

      params.push(id);
      if (sets.length > 0) {
        await client.query(
          `UPDATE catalog.productos SET ${sets.join(', ')} WHERE id = $${params.length}`,
          params,
        );
      }
    });

    if (previo.producto_padre_id) {
      await this.recalcularPadre(previo.producto_padre_id);
    }
    const actualizado = await this.encontrarPorId(id, empresaId);
    if (!actualizado) throw new NotFoundError('Producto', id);
    return actualizado;
  }

  async actualizarVariante(
    padreId: string,
    varianteId: string,
    cambios: {
      nombre?: string;
      descripcion?: string;
      categoria?: string;
      precio_base?: string | number;
      stock?: number;
      atributos?: Record<string, unknown>;
      motivo?: string;
    },
    empresaId: string,
  ): Promise<Producto> {
    const padre = await this.encontrarPorId(padreId, empresaId);
    if (!padre) throw new NotFoundError('Producto', padreId);
    if (!padre.variantes?.some((v) => v.id === varianteId)) {
      throw new NotFoundError('Variante', varianteId);
    }
    return this.actualizar(varianteId, cambios, empresaId);
  }

  async eliminarVariante(
    padreId: string,
    varianteId: string,
    empresaId: string,
  ): Promise<void> {
    const padre = await this.encontrarPorId(padreId, empresaId);
    if (!padre) throw new NotFoundError('Producto', padreId);
    if (!padre.variantes?.some((v) => v.id === varianteId)) {
      throw new NotFoundError('Variante', varianteId);
    }
    await this.validarSinOfertasActivas(varianteId);
    await this.pg.transaccion(async (client) => {
      await client.query(
        `DELETE FROM catalog.productos
          WHERE id = $1 AND empresa_id = $2 AND producto_padre_id = $3`,
        [varianteId, empresaId, padreId],
      );
      await client.query('SELECT catalog.fn_recalcular_padre($1)', [padreId]);
    });
  }

  /** Agrega una imagen (máx. 3) a un producto o variante. */
  async agregarImagen(
    productoId: string,
    imagen: ImagenProductoInput,
    empresaId: string,
  ): Promise<Producto> {
    const producto = await this.encontrarPorId(productoId, empresaId);
    if (!producto) throw new NotFoundError('Producto', productoId);
    await this.validarImagenes(empresaId, [imagen]);

    await this.pg.transaccion(async (client) => {
      const actuales = await client.query<ImagenRow>(
        `SELECT id, producto_id, objeto_id, orden, es_principal
           FROM catalog.producto_imagenes
          WHERE producto_id = $1
          ORDER BY orden
          FOR UPDATE`,
        [productoId],
      );
      if (actuales.rows.length >= MAX_IMAGENES) {
        throw new DomainError(
          'LIMITE_IMAGENES',
          `Cada producto o variante admite hasta ${MAX_IMAGENES} imágenes.`,
        );
      }
      const usados = new Set(actuales.rows.map((r) => r.orden));
      const orden = imagen.orden ?? this.siguienteOrden(usados);
      if (orden < 0 || orden >= MAX_IMAGENES || usados.has(orden)) {
        throw new DomainError('IMAGEN_ORDEN_INVALIDO', 'El orden de la imagen no es válido.');
      }
      const principalSolicitada = imagen.es_principal === true;
      if (principalSolicitada) {
        await client.query(
          `UPDATE catalog.producto_imagenes SET es_principal = false WHERE producto_id = $1`,
          [productoId],
        );
      }
      await client.query(
        `INSERT INTO catalog.producto_imagenes
           (id, empresa_id, producto_id, objeto_id, orden, es_principal)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [randomUUID(), empresaId, productoId, imagen.objeto_id, orden, principalSolicitada],
      );
      if (!principalSolicitada && !actuales.rows.some((r) => r.es_principal)) {
        await client.query(
          `UPDATE catalog.producto_imagenes
              SET es_principal = true
            WHERE id = (
              SELECT id FROM catalog.producto_imagenes
               WHERE producto_id = $1 ORDER BY orden LIMIT 1
            )`,
          [productoId],
        );
      }
    });

    const actualizado = await this.encontrarPorId(productoId, empresaId);
    if (!actualizado) throw new NotFoundError('Producto', productoId);
    return actualizado;
  }

  async eliminarImagen(
    productoId: string,
    imagenId: string,
    empresaId: string,
  ): Promise<void> {
    const producto = await this.encontrarPorId(productoId, empresaId);
    if (!producto) throw new NotFoundError('Producto', productoId);
    const imagen = producto.imagenes.find((i) => i.id === imagenId);
    if (!imagen) {
      throw new NotFoundError('Imagen', imagenId);
    }
    // La oferta congela el objeto de la imagen principal: si está viva, borrarla
    // dejaría una URL rota en la tienda pública.
    if (imagen.es_principal) {
      await this.validarSinOfertasActivas(productoId);
    }
    await this.pg.transaccion(async (client) => {
      const eliminada = await client.query<{ es_principal: boolean }>(
        `DELETE FROM catalog.producto_imagenes
          WHERE id = $1 AND producto_id = $2 AND empresa_id = $3
          RETURNING es_principal`,
        [imagenId, productoId, empresaId],
      );
      if (eliminada.rows[0]?.es_principal) {
        await client.query(
          `UPDATE catalog.producto_imagenes
              SET es_principal = true
            WHERE id = (
              SELECT id FROM catalog.producto_imagenes
               WHERE producto_id = $1 ORDER BY orden LIMIT 1
            )`,
          [productoId],
        );
      }
    });
  }

  /**
   * RN-03: descuento atómico de stock dentro de una transacción.
   * Si alguna línea no alcanza stock, se rechaza la reserva completa (la
   * transacción revierte las líneas ya descontadas). La clave es (empresa, SKU):
   * cada empresa tiene su propio stock (ADR-27) y cada variante su propio SKU.
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
          const lock = await client.query<{ stock: number; tiene_variantes: boolean }>(
            `SELECT p.stock,
                    EXISTS (SELECT 1 FROM catalog.productos v WHERE v.producto_padre_id = p.id) AS tiene_variantes
               FROM catalog.productos p
              WHERE p.empresa_id = $1 AND LOWER(p.sku) = LOWER($2) FOR UPDATE`,
            [item.empresa_id, item.sku],
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
          if (lock.rows[0].tiene_variantes) {
            fallidos.push({
              empresa_id: item.empresa_id,
              sku: item.sku,
              cantidad: item.cantidad,
              motivo: 'variante_requerida',
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
      [
        JSON.stringify([
          ...new Map(
            items.map((i) => [
              `${i.empresa_id}:${i.sku.toLowerCase()}`,
              {
                empresa_id: i.empresa_id,
                sku: String(i.sku).toLowerCase(),
              },
            ]),
          ).values(),
        ]),
      ],
    );
    return { ok: true, fallidos, stock_restante: stock };
  }

  /** RN-06: devolución aprobada revierte stock por (empresa, SKU). */
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
      await client.query('DELETE FROM catalog.reservas_ordenes WHERE order_id = $1', [orderId]);
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
    const filtro = empresaId
      ? 'WHERE p.empresa_id = $1 AND NOT EXISTS (SELECT 1 FROM catalog.productos v WHERE v.producto_padre_id = p.id)'
      : 'WHERE NOT EXISTS (SELECT 1 FROM catalog.productos v WHERE v.producto_padre_id = p.id)';
    const params = empresaId ? [empresaId] : [];
    return this.pg
      .queryOne<{
        productos: number;
        agotados: number;
        stock_total: number;
        valor_inventario_cents: number;
      }>(
        `SELECT COUNT(*)::int AS productos,
                COUNT(*) FILTER (WHERE p.estado = 'agotado')::int AS agotados,
                COALESCE(SUM(p.stock), 0)::int AS stock_total,
                COALESCE(SUM(p.stock * p.precio_base_cents), 0)::int AS valor_inventario_cents
           FROM catalog.productos p ${filtro}`,
        params,
      )
      .then(
        (r) => r ?? { productos: 0, agotados: 0, stock_total: 0, valor_inventario_cents: 0 },
      );
  }

  // ── helpers ────────────────────────────────────────────────────────────

  private async detallar(fila: ProductoRow): Promise<Producto> {
    const hijos =
      fila.producto_padre_id === null || fila.producto_padre_id === undefined
        ? await this.pg.query<ProductoRow>(
            `SELECT v.id, v.empresa_id, v.sku, v.nombre, v.descripcion, v.categoria,
                    v.precio_base_cents, v.disponible AS stock, v.estado, v.creado_en,
                    v.producto_padre_id, v.atributos, false AS tiene_variantes
               FROM catalog.producto_catalogo_vista v
              WHERE v.producto_padre_id = $1
              ORDER BY v.creado_en ASC`,
            [fila.id],
          )
        : [];
    const imagenes = await this.imagenesDe([fila.id, ...hijos.map((h) => h.id)]);
    const variantes = hijos.map((h) => this.serializar(h, imagenes.get(h.id) ?? []));
    return this.serializar(fila, imagenes.get(fila.id) ?? [], variantes);
  }

  private async imagenesDe(ids: string[]): Promise<Map<string, ImagenRow[]>> {
    const mapa = new Map<string, ImagenRow[]>();
    if (!ids.length) return mapa;
    const filas = await this.pg.query<ImagenRow>(
      `SELECT id, producto_id, objeto_id, orden, es_principal
         FROM catalog.producto_imagenes
        WHERE producto_id = ANY($1::uuid[])
        ORDER BY producto_id, orden`,
      [ids],
    );
    for (const fila of filas) {
      const lista = mapa.get(fila.producto_id) ?? [];
      lista.push(fila);
      mapa.set(fila.producto_id, lista);
    }
    return mapa;
  }

  private serializar(
    fila: ProductoRow,
    imagenes: ImagenRow[] = [],
    variantes: Producto[] = [],
  ): Producto {
    const precio = Money.desdeCentavos(fila.precio_base_cents).string();
    return {
      id: fila.id,
      empresa_id: fila.empresa_id,
      sku: fila.sku,
      nombre: fila.nombre,
      descripcion: fila.descripcion,
      categoria: fila.categoria,
      precio_base: precio,
      stock: fila.stock,
      estado: fila.estado === 'disponible' ? 'disponible' : 'agotado',
      creado_en: fila.creado_en,
      producto_padre_id: fila.producto_padre_id ?? null,
      atributos: fila.atributos ?? {},
      tiene_variantes: fila.tiene_variantes === true || variantes.length > 0,
      imagenes: imagenes.map((i) => this.serializarImagen(i)),
      ...(variantes.length > 0 ? { variantes } : {}),
      precio_desde: precio,
      stock_total: fila.stock,
    };
  }

  private serializarImagen(fila: ImagenRow): ImagenProducto {
    return {
      id: fila.id,
      objeto_id: fila.objeto_id,
      url: `${URL_IMAGEN_PUBLICA}/${fila.objeto_id}`,
      orden: fila.orden,
      es_principal: fila.es_principal,
    };
  }

  private normalizarSku(valor: string): string {
    const sku = String(valor).trim().toUpperCase();
    if (!/^[A-Z0-9-]{2,32}$/.test(sku)) {
      throw new DomainError('SKU_INVALIDO', 'El SKU solo admite letras, numeros y guiones (2-32).');
    }
    return sku;
  }

  private validarStock(stock: number): void {
    if (stock < 0 || !Number.isInteger(stock)) {
      throw new DomainError('STOCK_INVALIDO', 'El stock debe ser un entero mayor o igual a 0.');
    }
  }

  private validarPrecio(valor: string | number): Money {
    const precio = Money.parsear(valor);
    if (!precio.esPositivo()) {
      throw new DomainError('PRECIO_INVALIDO', 'El precio base debe ser mayor a 0.');
    }
    return precio;
  }

  private async skuExiste(empresaId: string, sku: string): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM catalog.productos WHERE empresa_id = $1 AND LOWER(sku) = LOWER($2)`,
      [empresaId, sku],
    );
    return !!fila;
  }

  private async registrarHistorico(
    client: PoolClient,
    empresaId: string,
    sku: string,
    anterior: number | null,
    nuevo: number,
  ): Promise<void> {
    await client.query(
      `INSERT INTO catalog.historico_precios
         (id, empresa_id, producto_sku, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
       VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW())`,
      [empresaId, sku, anterior, nuevo],
    );
  }

  private async recalcularPadre(padreId: string): Promise<void> {
    await this.pg.query('SELECT catalog.fn_recalcular_padre($1)', [padreId]);
  }

  /**
   * ADR-28: no se puede crear/eliminar una variante ni borrar la imagen
   * principal mientras exista una oferta activa que la referencie. La consulta
   * es a stores por API interna y falla cerrada si no se puede verificar.
   */
  private async validarSinOfertasActivas(productoId: string): Promise<void> {
    let res: Response;
    try {
      res = await fetch(
        `${this.storesUrl}/internal/ofertas/producto/${encodeURIComponent(productoId)}/activas`,
        {
          headers: { 'x-internal-key': this.claveInterna },
          signal: AbortSignal.timeout(4000),
        },
      );
    } catch {
      throw new DomainError(
        'OFERTAS_NO_VERIFICABLES',
        'No se pudo verificar si el producto tiene ofertas activas.',
      );
    }
    if (!res.ok) {
      throw new DomainError(
        'OFERTAS_NO_VERIFICABLES',
        'No se pudo verificar si el producto tiene ofertas activas.',
      );
    }
    const data = (await res.json()) as { total?: number };
    if ((data.total ?? 0) > 0) {
      throw new DomainError(
        'OFERTAS_ACTIVAS',
        'El producto tiene ofertas activas; desactívalas antes de cambiar sus variantes o imágenes.',
      );
    }
  }

  /**
   * ADR-27: solo una empresa activa de tipo comercial tiene catálogo. Se
   * consulta a identity por API interna (R-DB-1) y se falla cerrado si no se
   * puede verificar.
   */
  private async validarEmpresaCatalogo(empresaId: string): Promise<void> {
    if (!empresaId || empresaId === EMPRESA_LEGACY_ID) {
      throw new DomainError(
        'EMPRESA_NO_PUBLICA',
        'La empresa comodín del catálogo legado no publica productos.',
      );
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

  private async validarImagenes(
    empresaId: string,
    imagenes: ImagenProductoInput[],
  ): Promise<void> {
    if (imagenes.length > MAX_IMAGENES) {
      throw new DomainError(
        'LIMITE_IMAGENES',
        `Cada producto o variante admite hasta ${MAX_IMAGENES} imágenes.`,
      );
    }
    const ordenes = new Set<number>();
    let principales = 0;
    for (let i = 0; i < imagenes.length; i += 1) {
      const imagen = imagenes[i];
      if (!UUID.test(imagen.objeto_id)) {
        throw new DomainError('IMAGEN_INVALIDA', 'El objeto de la imagen no es un UUID válido.');
      }
      const orden = imagen.orden ?? this.siguienteOrden(ordenes);
      if (orden < 0 || orden >= MAX_IMAGENES || ordenes.has(orden)) {
        throw new DomainError('IMAGEN_ORDEN_INVALIDO', 'El orden de la imagen no es válido.');
      }
      ordenes.add(orden);
      if (imagen.es_principal) principales += 1;
    }
    if (principales > 1) {
      throw new DomainError('IMAGEN_PRINCIPAL_UNICA', 'Solo puede haber una imagen principal.');
    }
    const unicos = [...new Set(imagenes.map((i) => i.objeto_id))];
    for (const objetoId of unicos) {
      await this.validarObjetoImagen(empresaId, objetoId);
    }
  }

  private async validarObjetoImagen(empresaId: string, objetoId: string): Promise<void> {
    let res: Response;
    try {
      res = await fetch(
        `${this.storageUrl}/internal/storage/objetos/${encodeURIComponent(objetoId)}?tenant_id=${encodeURIComponent(empresaId)}`,
        {
          headers: { 'x-internal-key': this.claveInterna },
          signal: AbortSignal.timeout(4000),
        },
      );
    } catch {
      throw new DomainError('IMAGEN_NO_VERIFICABLE', 'No se pudo verificar la imagen en storage.');
    }
    if (res.status === 404) {
      throw new DomainError('IMAGEN_NO_ENCONTRADA', 'La imagen no existe en el almacenamiento.');
    }
    if (!res.ok) {
      throw new DomainError('IMAGEN_NO_VERIFICABLE', 'No se pudo verificar la imagen en storage.');
    }
    const objeto = (await res.json()) as {
      tenant_id?: string;
      estado?: string;
      content_type?: string;
      publico?: boolean;
    };
    if (objeto.tenant_id !== empresaId) {
      throw new DomainError('IMAGEN_OTRA_EMPRESA', 'La imagen pertenece a otra empresa.');
    }
    if (objeto.estado !== 'activo') {
      throw new DomainError('IMAGEN_NO_ACTIVA', 'La imagen debe estar confirmada y activa.');
    }
    if (
      !objeto.content_type?.startsWith('image/') ||
      objeto.content_type === 'image/svg+xml'
    ) {
      throw new DomainError('IMAGEN_TIPO_INVALIDO', 'El objeto no es una imagen raster permitida.');
    }
    if (objeto.publico !== true) {
      throw new DomainError(
        'IMAGEN_NO_PUBLICA',
        'La imagen debe subirse con publico=true para servirse en el catálogo.',
      );
    }
  }

  private siguienteOrden(usados: Set<number>): number {
    for (let i = 0; i < MAX_IMAGENES; i += 1) {
      if (!usados.has(i)) return i;
    }
    return -1;
  }

  private async insertarImagenes(
    client: PoolClient,
    empresaId: string,
    productoId: string,
    imagenes: ImagenProductoInput[],
  ): Promise<void> {
    if (!imagenes.length) return;
    const usados = new Set<number>();
    const hayPrincipal = imagenes.some((i) => i.es_principal === true);
    for (let i = 0; i < imagenes.length; i += 1) {
      const imagen = imagenes[i];
      const orden = imagen.orden ?? this.siguienteOrden(usados);
      if (orden < 0 || orden >= MAX_IMAGENES || usados.has(orden)) {
        throw new DomainError('IMAGEN_ORDEN_INVALIDO', 'El orden de la imagen no es válido.');
      }
      usados.add(orden);
      await client.query(
        `INSERT INTO catalog.producto_imagenes
           (id, empresa_id, producto_id, objeto_id, orden, es_principal)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [randomUUID(), empresaId, productoId, imagen.objeto_id, orden, imagen.es_principal === true],
      );
    }
    if (!hayPrincipal) {
      await client.query(
        `UPDATE catalog.producto_imagenes
            SET es_principal = true
          WHERE id = (
            SELECT id FROM catalog.producto_imagenes
             WHERE producto_id = $1 ORDER BY orden LIMIT 1
          )`,
        [productoId],
      );
    }
  }
}
