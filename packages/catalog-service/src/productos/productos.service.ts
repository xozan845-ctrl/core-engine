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
  capacidadesDeEmpresas,
  timeoutInternoMs,
} from '@core/shared';
import { ItemsService } from '../items/items.service';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const MAX_IMAGENES = 3;
const MAX_VARIANTES = 50;
const URL_IMAGEN_PUBLICA = '/api/v2/storage/public/objetos';

/** Columnas de lectura compartidas por listado, detalle y variantes. */
const COLUMNAS_PRODUCTO = `i.id, i.empresa_id, i.codigo AS sku, i.nombre, i.descripcion,
  i.categoria, i.precio_cents, i.precio_cents AS precio_base_cents, i.costo_cents,
  i.marca, i.codigo_barras, i.unidad, i.permite_fracciones, i.unidad_compra,
  i.contenido_cantidad, i.contenido_unidad, i.tipo_precio, i.tipo_costo,
  i.stock_minimo, i.stock_maximo, i.garantia_meses, i.es_materia_prima, i.ubicacion,
  i.actualizado_en, i.stock, i.estado, i.creado_en,
  i.parent_item_id AS producto_padre_id, i.atributos,
  COALESCE((SELECT MIN(c.precio_cents) FROM catalog.items c
             WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL),
           i.precio_cents) AS precio_desde_cents,
  COALESCE((SELECT SUM(c.stock) FROM catalog.items c
             WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL),
           i.stock) AS stock_total`;

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
  /** Alta/baja comercial independiente del stock (R-PS-03). */
  estado_operativo: 'activo' | 'inactivo';
  costo: string;
  marca: string;
  codigo_barras: string;
  unidad: string;
  permite_fracciones: boolean;
  unidad_compra: string;
  contenido_cantidad: number;
  contenido_unidad: string;
  tipo_precio: 'fijo' | 'escala';
  tipo_costo: 'fijo' | 'escala';
  stock_minimo: number;
  stock_maximo: number | null;
  garantia_meses: number | null;
  es_materia_prima: boolean;
  ubicacion: string;
  actualizado_en: string;
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

/** Metadatos operativos del producto (R-PS-01). */
export interface MetadatosProductoInput {
  costo?: string | number;
  marca?: string;
  codigo_barras?: string;
  unidad?: string;
  permite_fracciones?: boolean;
  unidad_compra?: string;
  contenido_cantidad?: number;
  contenido_unidad?: string;
  tipo_precio?: 'fijo' | 'escala';
  tipo_costo?: 'fijo' | 'escala';
  stock_minimo?: number;
  stock_maximo?: number | null;
  garantia_meses?: number | null;
  es_materia_prima?: boolean;
  ubicacion?: string;
  estado_operativo?: 'activo' | 'inactivo';
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
  actualizado_en?: string;
  costo_cents?: number;
  marca?: string;
  codigo_barras?: string;
  unidad?: string;
  permite_fracciones?: boolean;
  unidad_compra?: string;
  contenido_cantidad?: number;
  contenido_unidad?: string;
  tipo_precio?: string;
  tipo_costo?: string;
  stock_minimo?: number;
  stock_maximo?: number | null;
  garantia_meses?: number | null;
  es_materia_prima?: boolean;
  ubicacion?: string;
  producto_padre_id?: string | null;
  atributos?: Record<string, unknown> | null;
  tiene_variantes?: boolean;
  precio_desde_cents?: number;
  stock_total?: number;
}

interface ImagenRow {
  id: string;
  item_id: string;
  objeto_id: string;
  orden: number;
  es_principal: boolean;
}

interface EmpresaCatalogo {
  estado: string;
}

interface MetadatosNormalizados {
  costo_cents: number;
  marca: string;
  codigo_barras: string;
  unidad: string;
  permite_fracciones: boolean;
  unidad_compra: string;
  contenido_cantidad: number;
  contenido_unidad: string;
  tipo_precio: 'fijo' | 'escala';
  tipo_costo: 'fijo' | 'escala';
  stock_minimo: number;
  stock_maximo: number | null;
  garantia_meses: number | null;
  es_materia_prima: boolean;
  ubicacion: string;
  estado_operativo: 'activo' | 'inactivo';
}

/**
 * Catálogo maestro **por empresa** (ADR-27) con variantes e imágenes (ADR-28).
 * Tras la Fase 3 todo el catálogo vive en `catalog.items`: un producto es un
 * item `tipo='producto'`, una variante es un item hijo (`parent_item_id`) y las
 * imágenes se guardan en `catalog.item_imagenes`. Las reservas/órdenes siguen
 * operando por (empresa, código) y el padre agrega stock y precio mínimo.
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
    @Optional() private readonly items?: ItemsService,
  ) {}

  async listar(filtros: {
    empresa_id?: unknown;
    pagina?: unknown;
    limite?: unknown;
    q?: unknown;
    estado?: unknown;
    categoria?: unknown;
    incluir_inactivos?: unknown;
  }): Promise<Pagina<Producto>> {
    const empresaId = this.exigirEmpresa(filtros.empresa_id);
    const { pagina, limite } = parsearPaginacion(filtros as Record<string, unknown>);
    const params: unknown[] = [empresaId];
    const condiciones: string[] = [
      'i.empresa_id = $1',
      "i.tipo = 'producto'",
      'i.parent_item_id IS NULL',
      'i.eliminado_en IS NULL',
    ];
    if (filtros.incluir_inactivos !== true) {
      condiciones.push(`i.estado = 'activo'`);
    }
    if (filtros.q) {
      params.push(`%${String(filtros.q).toLowerCase()}%`);
      condiciones.push(
        `(LOWER(i.nombre) LIKE $${params.length} OR LOWER(i.codigo) LIKE $${params.length})`,
      );
    }
    if (filtros.estado) {
      condiciones.push(
        String(filtros.estado) === 'disponible' ? 'i.stock > 0' : 'i.stock = 0',
      );
    }
    if (filtros.categoria) {
      params.push(String(filtros.categoria));
      condiciones.push(`LOWER(i.categoria) = LOWER($${params.length})`);
    }
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(
        `SELECT COUNT(*)::int AS n FROM catalog.items i ${where}`,
        params,
      )
      .then((r) => r?.n ?? 0);
    const offset = (pagina - 1) * limite;
    const filas = await this.pg.query<ProductoRow>(
      `SELECT ${COLUMNAS_PRODUCTO},
              EXISTS (SELECT 1 FROM catalog.items c
                       WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL) AS tiene_variantes
       FROM catalog.items i ${where}
       ORDER BY i.creado_en DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, offset],
    );
    const imagenes = await this.imagenesDe(filas.map((f) => f.id));
    return crearPagina(
      filas.map((f) => this.serializar(f, imagenes.get(f.id) ?? [])),
      total,
      { pagina, limite },
    );
  }

  async encontrarPorId(
    id: string,
    empresaId?: string,
    incluirInactivos = false,
  ): Promise<Producto | null> {
    if (!UUID.test(id)) return null;
    const params: unknown[] = [id];
    let where = "i.id = $1 AND i.tipo = 'producto' AND i.eliminado_en IS NULL";
    if (empresaId) {
      params.push(empresaId);
      where += ` AND i.empresa_id = $${params.length}`;
    }
    if (!incluirInactivos) where += ` AND i.estado = 'activo'`;
    const fila = await this.pg.queryOne<ProductoRow>(
      `SELECT ${COLUMNAS_PRODUCTO},
              EXISTS (SELECT 1 FROM catalog.items c
                       WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL) AS tiene_variantes
       FROM catalog.items i WHERE ${where}`,
      params,
    );
    return fila ? this.detallar(fila) : null;
  }

  async encontrarPorSku(
    sku: string,
    empresaId?: string,
    incluirInactivos = false,
  ): Promise<Producto | null> {
    const params: unknown[] = [sku];
    let where = "LOWER(i.codigo) = LOWER($1) AND i.tipo = 'producto' AND i.eliminado_en IS NULL";
    if (empresaId) {
      params.push(empresaId);
      where += ` AND i.empresa_id = $${params.length}`;
    }
    if (!incluirInactivos) where += ` AND i.estado = 'activo'`;
    const fila = await this.pg.queryOne<ProductoRow>(
      `SELECT ${COLUMNAS_PRODUCTO},
              EXISTS (SELECT 1 FROM catalog.items c
                       WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL) AS tiene_variantes
       FROM catalog.items i WHERE ${where}`,
      params,
    );
    return fila ? this.detallar(fila) : null;
  }

  async encontrarPorCodigoBarras(
    codigo: string,
    empresaId: string,
    incluirInactivos = false,
  ): Promise<Producto | null> {
    const codigoBarras = codigo.trim();
    if (!codigoBarras) return null;
    const fila = await this.pg.queryOne<ProductoRow>(
      `SELECT ${COLUMNAS_PRODUCTO},
              EXISTS (SELECT 1 FROM catalog.items c
                       WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL) AS tiene_variantes
       FROM catalog.items i
       WHERE i.empresa_id = $1 AND i.codigo_barras = $2
         AND i.tipo = 'producto' AND i.eliminado_en IS NULL
         ${incluirInactivos ? '' : "AND i.estado = 'activo'"}
       LIMIT 1`,
      [empresaId, codigoBarras],
    );
    return fila ? this.detallar(fila) : null;
  }

  /**
   * Consulta por lote de códigos. Un código puede repetirse en empresas
   * distintas, por eso la fila incluye `empresa_id`: el consumidor debe
   * emparejar la clave compuesta (ADR-27).
   */
  async lotePorSkus(
    skus: string[],
  ): Promise<{ empresa_id: string; sku: string; stock: number; estado: string }[]> {
    if (!skus.length) return [];
    const params: string[] = [...new Set(skus.map((s) => String(s).toLowerCase()))];
    const marcadores = params.map((_, i) => `$${i + 1}`).join(', ');
    return this.pg.query<{ empresa_id: string; sku: string; stock: number; estado: string }>(
      `SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.stock,
              CASE WHEN i.stock > 0 THEN 'disponible' ELSE 'agotado' END AS estado
       FROM catalog.items i
       WHERE LOWER(i.codigo) IN (${marcadores})
         AND i.tipo = 'producto' AND i.estado = 'activo' AND i.eliminado_en IS NULL
       ORDER BY i.creado_en DESC`,
      params,
    );
  }

  /** Stock restante por producto (alimenta stock.updated para RN-02). */
  async stockActualDe(
    items: ClaveProducto[],
    client?: PoolClient,
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
    const sql = `SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.stock AS stock_restante
       FROM catalog.items i
       JOIN jsonb_to_recordset($1::jsonb) AS x(empresa_id uuid, sku text)
         ON i.empresa_id = x.empresa_id AND LOWER(i.codigo) = LOWER(x.sku)
      WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL`;
    if (client) {
      return (await client.query<{ empresa_id: string; sku: string; stock_restante: number }>(sql, [JSON.stringify(claves)])).rows;
    }
    return this.pg.query<{ empresa_id: string; sku: string; stock_restante: number }>(
      sql,
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
  } & MetadatosProductoInput, opciones?: { empresaValidada?: boolean }): Promise<Producto> {
    const sku = this.normalizarSku(datos.sku);
    this.validarStock(datos.stock);
    const precio = this.validarPrecio(datos.precio_base);
    const metadatos = this.normalizarMetadatos(datos);
    this.validarMetadatos(metadatos);
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
    if (!opciones?.empresaValidada) await this.validarEmpresaCatalogo(datos.empresa_id);
    await this.items?.validarAtributos(
      datos.empresa_id,
      'producto',
      datos.categoria ?? 'general',
      datos.atributos ?? {},
    );
    await this.validarCodigoBarras(datos.empresa_id, metadatos.codigo_barras);
    await this.validarImagenes(datos.empresa_id, datos.imagenes ?? []);
    for (const variante of variantes) {
      await this.items?.validarAtributos(
        datos.empresa_id,
        'producto',
        datos.categoria ?? 'general',
        variante.atributos ?? {},
      );
      await this.validarImagenes(datos.empresa_id, variante.imagenes ?? []);
    }

    const padreId = randomUUID();
    await this.pg.transaccion(async (client) => {
      await client.query(
        `INSERT INTO catalog.items
           (id, empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
            costo_cents, marca, codigo_barras, unidad, permite_fracciones,
            unidad_compra, contenido_cantidad, contenido_unidad, tipo_precio,
            tipo_costo, stock_minimo, stock_maximo, garantia_meses,
            es_materia_prima, ubicacion, estado, track_inventory, stock,
            parent_item_id, atributos, actualizado_en)
         VALUES ($1, $2, 'producto', $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                 $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, true, $24,
                 NULL, $25::jsonb, NOW())`,
        [
          padreId,
          datos.empresa_id,
          sku,
          datos.nombre.trim(),
          (datos.descripcion ?? '').trim(),
          (datos.categoria ?? 'general').trim(),
          precio.centavos,
          metadatos.costo_cents,
          metadatos.marca,
          metadatos.codigo_barras,
          metadatos.unidad,
          metadatos.permite_fracciones,
          metadatos.unidad_compra,
          metadatos.contenido_cantidad,
          metadatos.contenido_unidad,
          metadatos.tipo_precio,
          metadatos.tipo_costo,
          metadatos.stock_minimo,
          metadatos.stock_maximo,
          metadatos.garantia_meses,
          metadatos.es_materia_prima,
          metadatos.ubicacion,
          metadatos.estado_operativo,
          datos.stock,
          JSON.stringify(datos.atributos ?? {}),
        ],
      );
      await this.registrarHistorico(client, datos.empresa_id, padreId, null, precio.centavos);

      for (const variante of variantes) {
        const varianteId = randomUUID();
        const varianteSku = this.normalizarSku(variante.sku);
        const variantePrecio = this.validarPrecio(variante.precio_base);
        await client.query(
          `INSERT INTO catalog.items
             (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
              precio_cents, track_inventory, stock, parent_item_id, atributos)
           VALUES ($1, $2, 'producto', $3, $4, $5, $6, $7, true, $8, $9, $10::jsonb)`,
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
          varianteId,
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
        await this.recalcularPadre(client, padreId);
      }
    });

    const creado = await this.encontrarPorId(padreId, datos.empresa_id, true);
    if (!creado) throw new NotFoundError('Producto', padreId);
    return creado;
  }

  /** Agrega una variante a un producto padre (ADR-28). */
  async crearVariante(
    padreId: string,
    datos: VarianteProductoInput,
    empresaId: string,
  ): Promise<Producto> {
    const padre = await this.encontrarPorId(padreId, empresaId, true);
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
    await this.items?.validarAtributos(
      empresaId,
      'producto',
      padre.categoria,
      datos.atributos ?? {},
    );
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
        `INSERT INTO catalog.items
           (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
            precio_cents, track_inventory, stock, parent_item_id, atributos)
         VALUES ($1, $2, 'producto', $3, $4, $5, $6, $7, true, $8, $9, $10::jsonb)`,
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
      await this.registrarHistorico(client, empresaId, varianteId, null, precio.centavos);
      await this.insertarImagenes(client, empresaId, varianteId, datos.imagenes ?? []);
      await this.recalcularPadre(client, padreId);
    });

    const creada = await this.encontrarPorId(varianteId, empresaId, true);
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
    } & MetadatosProductoInput,
    empresaId?: string,
  ): Promise<Producto> {
    const previo = await this.encontrarPorId(id, empresaId, true);
    if (!previo) throw new NotFoundError('Producto', id);
    if (previo.tiene_variantes && (cambios.precio_base !== undefined || cambios.stock !== undefined)) {
      throw new DomainError(
        'PRODUCTO_CON_VARIANTES',
        'El precio y el stock de un producto con variantes se editan en cada variante.',
      );
    }
    const metadatos = this.combinarMetadatos(previo, cambios);
    this.validarMetadatos(metadatos);
    if (cambios.atributos !== undefined) {
      await this.items?.validarAtributos(
        previo.empresa_id,
        'producto',
        cambios.categoria ?? previo.categoria,
        cambios.atributos,
      );
    }
    await this.validarCodigoBarras(previo.empresa_id, metadatos.codigo_barras, id);

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

      push('costo_cents = $', metadatos.costo_cents);
      push('marca = $', metadatos.marca);
      push('codigo_barras = $', metadatos.codigo_barras);
      push('unidad = $', metadatos.unidad);
      push('permite_fracciones = $', metadatos.permite_fracciones);
      push('unidad_compra = $', metadatos.unidad_compra);
      push('contenido_cantidad = $', metadatos.contenido_cantidad);
      push('contenido_unidad = $', metadatos.contenido_unidad);
      push('tipo_precio = $', metadatos.tipo_precio);
      push('tipo_costo = $', metadatos.tipo_costo);
      push('stock_minimo = $', metadatos.stock_minimo);
      push('stock_maximo = $', metadatos.stock_maximo);
      push('garantia_meses = $', metadatos.garantia_meses);
      push('es_materia_prima = $', metadatos.es_materia_prima);
      push('ubicacion = $', metadatos.ubicacion);
      push('estado = $', metadatos.estado_operativo);
      sets.push('actualizado_en = NOW()');

      if (cambios.atributos !== undefined) {
        params.push(JSON.stringify(cambios.atributos));
        sets.push(`atributos = $${params.length}::jsonb`);
      }

      if (cambios.precio_base !== undefined) {
        const nuevo = this.validarPrecio(cambios.precio_base);
        push('precio_cents = $', nuevo.centavos);
        await this.registrarHistorico(
          client,
          previo.empresa_id,
          previo.id,
          Money.parsear(previo.precio_base).centavos,
          nuevo.centavos,
        );
      }

      if (cambios.stock !== undefined) {
        this.validarStock(cambios.stock);
        push('stock = $', cambios.stock);
        await client.query(
          `INSERT INTO catalog.ajustes_stock
             (empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
           VALUES ($1, $2, $3, $4, $5, NOW())`,
          [
            previo.empresa_id,
            previo.id,
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
          `UPDATE catalog.items SET ${sets.join(', ')} WHERE id = $${params.length}`,
          params,
        );
      }
      if (previo.producto_padre_id) {
        await this.recalcularPadre(client, previo.producto_padre_id);
      }
    });

    const actualizado = await this.encontrarPorId(id, empresaId, true);
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
    const padre = await this.encontrarPorId(padreId, empresaId, true);
    if (!padre) throw new NotFoundError('Producto', padreId);
    if (!padre.variantes?.some((v) => v.id === varianteId)) {
      throw new NotFoundError('Variante', varianteId);
    }
    return this.actualizar(varianteId, cambios, empresaId);
  }

  /** Baja lógica del producto y de sus variantes (R-DB-5). */
  async eliminar(id: string, empresaId?: string): Promise<void> {
    const previo = await this.encontrarPorId(id, empresaId, true);
    if (!previo) throw new NotFoundError('Producto', id);
    await this.validarSinOfertasActivas(id);
    await this.pg.transaccion(async (client) => {
      await client.query(
        `UPDATE catalog.items
            SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
          WHERE id = $1 AND tipo = 'producto' AND eliminado_en IS NULL
            ${empresaId ? 'AND empresa_id = $2' : ''}`,
        empresaId ? [id, empresaId] : [id],
      );
      await client.query(
        `UPDATE catalog.items
            SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
          WHERE parent_item_id = $1 AND eliminado_en IS NULL`,
        [id],
      );
    });
  }

  async eliminarVariante(
    padreId: string,
    varianteId: string,
    empresaId: string,
  ): Promise<void> {    const padre = await this.encontrarPorId(padreId, empresaId, true);
    if (!padre) throw new NotFoundError('Producto', padreId);
    if (!padre.variantes?.some((v) => v.id === varianteId)) {
      throw new NotFoundError('Variante', varianteId);
    }
    await this.validarSinOfertasActivas(varianteId);
    await this.pg.transaccion(async (client) => {
      await client.query(
        `UPDATE catalog.items
            SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
          WHERE id = $1 AND empresa_id = $2 AND parent_item_id = $3
            AND eliminado_en IS NULL`,
        [varianteId, empresaId, padreId],
      );
      await this.recalcularPadre(client, padreId);
    });
  }

  /** Agrega una imagen (máx. 3) a un producto o variante. */
  async agregarImagen(
    productoId: string,
    imagen: ImagenProductoInput,
    empresaId: string,
  ): Promise<Producto> {
    const producto = await this.encontrarPorId(productoId, empresaId, true);
    if (!producto) throw new NotFoundError('Producto', productoId);
    await this.validarImagenes(empresaId, [imagen]);

    await this.pg.transaccion(async (client) => {
      const actuales = await client.query<ImagenRow>(
        `SELECT id, item_id, objeto_id, orden, es_principal
           FROM catalog.item_imagenes
          WHERE item_id = $1
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
          `UPDATE catalog.item_imagenes SET es_principal = false WHERE item_id = $1`,
          [productoId],
        );
      }
      await client.query(
        `INSERT INTO catalog.item_imagenes
           (id, empresa_id, item_id, objeto_id, orden, es_principal)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [randomUUID(), empresaId, productoId, imagen.objeto_id, orden, principalSolicitada],
      );
      if (!principalSolicitada && !actuales.rows.some((r) => r.es_principal)) {
        await client.query(
          `UPDATE catalog.item_imagenes
              SET es_principal = true
            WHERE id = (
              SELECT id FROM catalog.item_imagenes
               WHERE item_id = $1 ORDER BY orden LIMIT 1
            )`,
          [productoId],
        );
      }
    });

    const actualizado = await this.encontrarPorId(productoId, empresaId, true);
    if (!actualizado) throw new NotFoundError('Producto', productoId);
    return actualizado;
  }

  async eliminarImagen(
    productoId: string,
    imagenId: string,
    empresaId: string,
  ): Promise<void> {
    const producto = await this.encontrarPorId(productoId, empresaId, true);
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
        `DELETE FROM catalog.item_imagenes
          WHERE id = $1 AND item_id = $2 AND empresa_id = $3
          RETURNING es_principal`,
        [imagenId, productoId, empresaId],
      );
      if (eliminada.rows[0]?.es_principal) {
        await client.query(
          `UPDATE catalog.item_imagenes
              SET es_principal = true
            WHERE id = (
              SELECT id FROM catalog.item_imagenes
               WHERE item_id = $1 ORDER BY orden LIMIT 1
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
    return this.pg.transaccion((client) => this.reservarStockEnTransaccion(client, orderId, items));
  }

  /**
   * Variante transaccional: el descuento de stock, la reserva y los eventos
   * que emite el consumidor comparten la misma transacción (saga atómica).
   */
  async reservarStockEnTransaccion(
    client: PoolClient,
    orderId: string,
    items: { empresa_id: string; sku: string; cantidad: number }[],
  ): Promise<{
    ok: boolean;
    fallidos: { empresa_id: string; sku: string; cantidad: number; motivo: string }[];
    stock_restante?: { empresa_id: string; sku: string; stock_restante: number }[];
  }> {
    const fallidos: { empresa_id: string; sku: string; cantidad: number; motivo: string }[] = [];
    await client.query('SAVEPOINT reserva_stock');
    try {
        for (const item of items) {
          // Bloqueo pesimista requerido por regla dorada (Data Integrity)
          const lock = await client.query<{ stock: number; item_id: string; tiene_variantes: boolean }>(
            `SELECT i.stock, i.id AS item_id,
                    EXISTS (SELECT 1 FROM catalog.items v
                             WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL) AS tiene_variantes
               FROM catalog.items i
              WHERE i.empresa_id = $1 AND LOWER(i.codigo) = LOWER($2)
                AND i.tipo = 'producto' AND i.estado = 'activo'
                AND i.eliminado_en IS NULL FOR UPDATE`,
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

          const itemId = lock.rows[0].item_id;
          const lotes = await client.query<{
            id: string;
            cantidad_disponible: number;
            vencido: boolean;
          }>(
            `SELECT id, cantidad_disponible,
                    (vence_en IS NOT NULL AND vence_en < CURRENT_DATE) AS vencido
               FROM catalog.lotes_inventario
              WHERE empresa_id = $1 AND item_id = $2 AND cantidad_disponible > 0
              ORDER BY vence_en NULLS LAST, creado_en, id
              FOR UPDATE`,
            [item.empresa_id, itemId],
          );
          if ((lotes.rowCount ?? 0) > 0) {
            let restante = item.cantidad;
            for (const lote of lotes.rows.filter((fila) => !fila.vencido)) {
              if (restante === 0) break;
              const cantidad = Math.min(restante, lote.cantidad_disponible);
              await client.query(
                `UPDATE catalog.lotes_inventario
                    SET cantidad_disponible = cantidad_disponible - $1,
                        actualizado_en = NOW()
                  WHERE id = $2`,
                [cantidad, lote.id],
              );
              await client.query(
                `INSERT INTO catalog.reservas_lotes
                   (order_id, lote_id, empresa_id, item_id, cantidad)
                 VALUES ($1, $2, $3, $4, $5)`,
                [orderId, lote.id, item.empresa_id, itemId, cantidad],
              );
              restante -= cantidad;
            }
            if (restante > 0) {
              fallidos.push({
                empresa_id: item.empresa_id,
                sku: item.sku,
                cantidad: item.cantidad,
                motivo: 'lote_vencido',
              });
              continue;
            }
          }

          await client.query(
            `UPDATE catalog.items
             SET stock = stock - $3, actualizado_en = NOW()
             WHERE empresa_id = $1 AND LOWER(codigo) = LOWER($2)
               AND tipo = 'producto' AND eliminado_en IS NULL
             RETURNING codigo`,
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
        await client.query('RELEASE SAVEPOINT reserva_stock');
    } catch (err) {
      if (!(err instanceof Error) || err.message !== 'STOCK_INSUFICIENTE') throw err;
      await client.query('ROLLBACK TO SAVEPOINT reserva_stock');
      await client.query('RELEASE SAVEPOINT reserva_stock');
      return { ok: false, fallidos };
    }
    const stock = await client.query<{ empresa_id: string; sku: string; stock_restante: number }>(
      `SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.stock AS stock_restante
       FROM catalog.items i
       JOIN jsonb_to_recordset($1::jsonb) AS x(empresa_id uuid, sku text)
         ON i.empresa_id = x.empresa_id AND LOWER(i.codigo) = LOWER(x.sku)
      WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL`,
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
    return { ok: true, fallidos, stock_restante: stock.rows };
  }

  /** RN-06: devolución aprobada revierte stock por (empresa, SKU). */
  async reintegrarStock(
    orderId: string,
    items: { empresa_id: string; sku: string; cantidad: number }[],
  ): Promise<void> {
    await this.pg.transaccion((client) => this.reintegrarStockEnTransaccion(client, orderId, items));
  }

  async reintegrarStockEnTransaccion(
    client: PoolClient,
    orderId: string,
    items: { empresa_id: string; sku: string; cantidad: number }[],
  ): Promise<void> {
      for (const item of items) {
        await client.query(
          `UPDATE catalog.items
           SET stock = stock + $3, actualizado_en = NOW()
           WHERE empresa_id = $1 AND LOWER(codigo) = LOWER($2)
             AND tipo = 'producto' AND eliminado_en IS NULL`,
          [item.empresa_id, item.sku, item.cantidad],
        );
      }
      await client.query(
        `INSERT INTO catalog.ajustes_stock
           (empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
         SELECT i.empresa_id, i.id, i.stock - x.cantidad, i.stock,
                'devolucion orden ' || $1, NOW()
         FROM catalog.items i
         JOIN jsonb_to_recordset($2::jsonb) AS x(empresa_id uuid, sku text, cantidad int)
           ON i.empresa_id = x.empresa_id AND LOWER(i.codigo) = LOWER(x.sku)
        WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL`,
        [orderId, JSON.stringify(items)],
      );
      // Restaura exactamente las asignaciones FEFO de la reserva.
      await client.query(
        `UPDATE catalog.lotes_inventario l
            SET cantidad_disponible = LEAST(l.cantidad_inicial, l.cantidad_disponible + r.cantidad),
                actualizado_en = NOW()
           FROM catalog.reservas_lotes r
          WHERE r.lote_id = l.id AND r.order_id = $1`,
        [orderId],
      );
      await client.query('DELETE FROM catalog.reservas_lotes WHERE order_id = $1', [orderId]);
      // La reserva de esta orden ya no está activa tras reintegrar el stock.
      await client.query('DELETE FROM catalog.reservas_ordenes WHERE order_id = $1', [orderId]);
  }

  /** Claves de eventos ya procesadas (idempotencia, doc 5.2). */
  async estaProcesado(event_id: string, client?: PoolClient): Promise<boolean> {
    const fila = client
      ? (await client.query(`SELECT 1 FROM catalog.eventos_procesados WHERE event_id = $1`, [event_id])).rows[0]
      : await this.pg.queryOne(
          `SELECT 1 FROM catalog.eventos_procesados WHERE event_id = $1`,
          [event_id],
        );
    return !!fila;
  }

  async registrarProcesado(event_id: string, tipo: string, client?: PoolClient): Promise<void> {
    const sql = `INSERT INTO catalog.eventos_procesados (event_id, tipo, procesado_en)
       VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING`;
    if (client) {
      await client.query(sql, [event_id, tipo]);
      return;
    }
    await this.pg.query(sql, [event_id, tipo]);
  }

  resumenInventario(empresaId?: string): Promise<{
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

  // ── helpers ────────────────────────────────────────────────────────────

  private async detallar(fila: ProductoRow): Promise<Producto> {
    const hijos =
      fila.producto_padre_id === null || fila.producto_padre_id === undefined
        ? await this.pg.query<ProductoRow>(
            `SELECT ${COLUMNAS_PRODUCTO}, false AS tiene_variantes
               FROM catalog.items i
              WHERE i.parent_item_id = $1 AND i.tipo = 'producto' AND i.eliminado_en IS NULL
              ORDER BY i.creado_en ASC`,
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
      `SELECT id, item_id, objeto_id, orden, es_principal
         FROM catalog.item_imagenes
        WHERE item_id = ANY($1::uuid[])
        ORDER BY item_id, orden`,
      [ids],
    );
    for (const fila of filas) {
      const lista = mapa.get(fila.item_id) ?? [];
      lista.push(fila);
      mapa.set(fila.item_id, lista);
    }
    return mapa;
  }

  private serializar(
    fila: ProductoRow,
    imagenes: ImagenRow[] = [],
    variantes: Producto[] = [],
  ): Producto {
    const precio = Money.desdeCentavos(fila.precio_base_cents).string();
    const costo = Money.desdeCentavos(fila.costo_cents ?? 0).string();
    const precioDesde = Money.desdeCentavos(fila.precio_desde_cents ?? fila.precio_base_cents).string();
    return {
      id: fila.id,
      empresa_id: fila.empresa_id,
      sku: fila.sku,
      nombre: fila.nombre,
      descripcion: fila.descripcion,
      categoria: fila.categoria,
      precio_base: precio,
      stock: fila.stock,
      estado: fila.stock > 0 ? 'disponible' : 'agotado',
      estado_operativo: fila.estado === 'inactivo' ? 'inactivo' : 'activo',
      costo,
      marca: fila.marca ?? '',
      codigo_barras: fila.codigo_barras ?? '',
      unidad: fila.unidad ?? 'Unidad (u)',
      permite_fracciones: fila.permite_fracciones === true,
      unidad_compra: fila.unidad_compra ?? 'Igual que la de venta',
      contenido_cantidad: fila.contenido_cantidad ?? 0,
      contenido_unidad: fila.contenido_unidad ?? 'Sin contenido',
      tipo_precio: fila.tipo_precio === 'escala' ? 'escala' : 'fijo',
      tipo_costo: fila.tipo_costo === 'escala' ? 'escala' : 'fijo',
      stock_minimo: fila.stock_minimo ?? 0,
      stock_maximo: fila.stock_maximo ?? null,
      garantia_meses: fila.garantia_meses ?? null,
      es_materia_prima: fila.es_materia_prima === true,
      ubicacion: fila.ubicacion ?? '',
      actualizado_en: fila.actualizado_en ?? fila.creado_en,
      creado_en: fila.creado_en,
      producto_padre_id: fila.producto_padre_id ?? null,
      atributos: fila.atributos ?? {},
      tiene_variantes: fila.tiene_variantes === true || variantes.length > 0,
      imagenes: imagenes.map((i) => this.serializarImagen(i)),
      ...(variantes.length > 0 ? { variantes } : {}),
      precio_desde: precioDesde,
      stock_total: fila.stock_total ?? fila.stock,
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

  /** Normaliza los metadatos operativos con defaults estables (R-PS-01). */
  private normalizarMetadatos(datos: MetadatosProductoInput): MetadatosNormalizados {
    const costo = Money.parsear(datos.costo ?? '0');
    return {
      costo_cents: costo.centavos,
      marca: (datos.marca ?? '').trim(),
      codigo_barras: (datos.codigo_barras ?? '').trim(),
      unidad: (datos.unidad ?? 'Unidad (u)').trim() || 'Unidad (u)',
      permite_fracciones: datos.permite_fracciones === true,
      unidad_compra: (datos.unidad_compra ?? 'Igual que la de venta').trim() ||
        'Igual que la de venta',
      contenido_cantidad: datos.contenido_cantidad ?? 0,
      contenido_unidad: (datos.contenido_unidad ?? 'Sin contenido').trim() || 'Sin contenido',
      tipo_precio: datos.tipo_precio === 'escala' ? 'escala' : 'fijo',
      tipo_costo: datos.tipo_costo === 'escala' ? 'escala' : 'fijo',
      stock_minimo: datos.stock_minimo ?? 0,
      stock_maximo: datos.stock_maximo ?? null,
      garantia_meses: datos.garantia_meses ?? null,
      es_materia_prima: datos.es_materia_prima === true,
      ubicacion: (datos.ubicacion ?? '').trim(),
      estado_operativo: datos.estado_operativo === 'inactivo' ? 'inactivo' : 'activo',
    };
  }

  /** Combina los metadatos actuales con los campos enviados en el PATCH. */
  private combinarMetadatos(
    previo: Producto,
    cambios: MetadatosProductoInput,
  ): MetadatosNormalizados {
    return this.normalizarMetadatos({
      costo: cambios.costo ?? previo.costo,
      marca: cambios.marca ?? previo.marca,
      codigo_barras: cambios.codigo_barras ?? previo.codigo_barras,
      unidad: cambios.unidad ?? previo.unidad,
      permite_fracciones: cambios.permite_fracciones ?? previo.permite_fracciones,
      unidad_compra: cambios.unidad_compra ?? previo.unidad_compra,
      contenido_cantidad: cambios.contenido_cantidad ?? previo.contenido_cantidad,
      contenido_unidad: cambios.contenido_unidad ?? previo.contenido_unidad,
      tipo_precio: cambios.tipo_precio ?? previo.tipo_precio,
      tipo_costo: cambios.tipo_costo ?? previo.tipo_costo,
      stock_minimo: cambios.stock_minimo ?? previo.stock_minimo,
      stock_maximo: cambios.stock_maximo !== undefined ? cambios.stock_maximo : previo.stock_maximo,
      garantia_meses:
        cambios.garantia_meses !== undefined ? cambios.garantia_meses : previo.garantia_meses,
      es_materia_prima: cambios.es_materia_prima ?? previo.es_materia_prima,
      ubicacion: cambios.ubicacion ?? previo.ubicacion,
      estado_operativo: cambios.estado_operativo ?? previo.estado_operativo,
    });
  }

  private validarMetadatos(meta: MetadatosNormalizados): void {
    const enteros: [string, number][] = [
      ['contenido_cantidad', meta.contenido_cantidad],
      ['stock_minimo', meta.stock_minimo],
    ];
    for (const [campo, valor] of enteros) {
      if (!Number.isInteger(valor) || valor < 0) {
        throw new DomainError('METADATO_INVALIDO', `${campo} debe ser un entero mayor o igual a 0.`);
      }
    }
    if (meta.stock_maximo !== null && (!Number.isInteger(meta.stock_maximo) || meta.stock_maximo < 0)) {
      throw new DomainError(
        'METADATO_INVALIDO',
        'stock_maximo debe ser un entero mayor o igual a 0 o null.',
      );
    }
    if (meta.garantia_meses !== null && (!Number.isInteger(meta.garantia_meses) || meta.garantia_meses < 0)) {
      throw new DomainError(
        'METADATO_INVALIDO',
        'garantia_meses debe ser un entero mayor o igual a 0 o null.',
      );
    }
    if (meta.stock_maximo !== null && meta.stock_maximo < meta.stock_minimo) {
      throw new DomainError(
        'METADATO_INVALIDO',
        'stock_maximo no puede ser menor que stock_minimo.',
      );
    }
  }

  /** El código de barras es único por empresa cuando está informado. */
  private async validarCodigoBarras(
    empresaId: string,
    codigoBarras: string,
    excluirId?: string,
  ): Promise<void> {
    if (!codigoBarras) return;
    const params: unknown[] = [empresaId, codigoBarras];
    let where = 'empresa_id = $1 AND codigo_barras = $2 AND eliminado_en IS NULL';
    if (excluirId) {
      params.push(excluirId);
      where += ` AND id <> $${params.length}`;
    }
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM catalog.items WHERE ${where} LIMIT 1`,
      params,
    );
    if (fila) {
      throw new ConflictError(`El código de barras ${codigoBarras} ya está registrado.`);
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
      `SELECT 1 FROM catalog.items
        WHERE empresa_id = $1 AND tipo = 'producto'
          AND LOWER(codigo) = LOWER($2) AND eliminado_en IS NULL`,
      [empresaId, sku],
    );
    return !!fila;
  }

  private async registrarHistorico(
    client: PoolClient,
    empresaId: string,
    itemId: string,
    anterior: number | null,
    nuevo: number,
  ): Promise<void> {
    await client.query(
      `INSERT INTO catalog.historico_precios
         (id, empresa_id, item_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
       VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW())`,
      [empresaId, itemId, anterior, nuevo],
    );
  }

  /** Agrega stock y precio mínimo de las variantes al item padre. */
  private async recalcularPadre(client: PoolClient, padreId: string): Promise<void> {
    await client.query(
      `UPDATE catalog.items p
          SET stock = agregado.stock,
              precio_cents = agregado.precio,
              actualizado_en = NOW()
         FROM (
           SELECT SUM(c.stock)::int AS stock, MIN(c.precio_cents)::int AS precio
             FROM catalog.items c
            WHERE c.parent_item_id = $1 AND c.eliminado_en IS NULL
         ) agregado
        WHERE p.id = $1 AND agregado.stock IS NOT NULL`,
      [padreId],
    );
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
   * ADR-27: solo una empresa activa con la capacidad de catálogo tiene items.
   * Se consulta a identity por API interna (R-DB-1) y se falla cerrado si no se
   * puede verificar.
   */
  async validarEmpresaCatalogo(empresaId: string): Promise<void> {
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
        {
          headers: { 'x-internal-key': this.claveInterna },
          signal: AbortSignal.timeout(timeoutInternoMs()),
        },
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
    const capacidades = await capacidadesDeEmpresas([empresaId], ['catalogo_items']);
    if (capacidades === null) {
      throw new DomainError('EMPRESA_NO_VERIFICABLE', 'No se pudieron verificar las capacidades de la empresa.');
    }
    if (!capacidades.some((c) => c.empresa_id === empresaId && c.capacidad_codigo === 'catalogo_items' && c.activo)) {
      throw new DomainError('EMPRESA_SIN_CATALOGO', 'La empresa no tiene activa la capacidad de catálogo.');
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
    itemId: string,
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
        `INSERT INTO catalog.item_imagenes
           (id, empresa_id, item_id, objeto_id, orden, es_principal)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [randomUUID(), empresaId, itemId, imagen.objeto_id, orden, imagen.es_principal === true],
      );
    }
    if (!hayPrincipal) {
      await client.query(
        `UPDATE catalog.item_imagenes
            SET es_principal = true
          WHERE id = (
            SELECT id FROM catalog.item_imagenes
             WHERE item_id = $1 ORDER BY orden LIMIT 1
          )`,
        [itemId],
      );
    }
  }
}
