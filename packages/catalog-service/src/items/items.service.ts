import { Injectable } from '@nestjs/common';
import Ajv, { type ErrorObject, type JSONSchemaType } from 'ajv';
import {
  ConflictError,
  DomainError,
  Money,
  Pagina,
  PgService,
  crearPagina,
  esViolacionUnicidad,
  parsearPaginacion,
} from '@core/shared';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const CODIGO = /^[A-Z0-9-]{2,32}$/;
const TIPOS = ['producto', 'servicio', 'alquiler'] as const;

export type TipoItem = (typeof TIPOS)[number];

export interface ItemImagen {
  id: string;
  objeto_id: string;
  orden: number;
  es_principal: boolean;
}

export interface Item {
  item_id: string;
  empresa_id: string;
  tipo: TipoItem;
  codigo: string;
  nombre: string;
  descripcion: string;
  categoria: string;
  precio: string;
  costo: string;
  track_inventory: boolean;
  track_serial: boolean;
  stock: number | null;
  duration_minutes: number | null;
  atributos: Record<string, unknown>;
  parent_item_id: string | null;
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
  imagenes: ItemImagen[];
  estado: 'activo' | 'inactivo';
  creado_en: string;
  actualizado_en: string;
}

export interface MetadatosItemInput {
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
}

export interface CrearItemInput extends MetadatosItemInput {
  tipo: TipoItem;
  codigo: string;
  nombre: string;
  descripcion?: string;
  categoria?: string;
  precio: string | number;
  costo?: string | number;
  track_inventory?: boolean;
  stock?: number | null;
  duration_minutes?: number | null;
  atributos?: Record<string, unknown>;
  estado?: 'activo' | 'inactivo';
  parent_item_id?: string | null;
}

export interface CrearAlquilerInput {
  codigo: string;
  nombre: string;
  descripcion?: string;
  categoria?: string;
  precio: string | number;
  costo?: string | number;
  atributos?: Record<string, unknown>;
  estado?: 'activo' | 'inactivo';
}

interface ItemRow {
  item_id: string;
  empresa_id: string;
  tipo: TipoItem;
  codigo: string;
  nombre: string;
  descripcion: string;
  categoria: string;
  precio_cents: number;
  costo_cents: number;
  track_inventory: boolean;
  track_serial: boolean;
  stock: number | null;
  duration_minutes: number | null;
  atributos: Record<string, unknown> | null;
  parent_item_id: string | null;
  marca: string;
  codigo_barras: string;
  unidad: string;
  permite_fracciones: boolean;
  unidad_compra: string;
  contenido_cantidad: number;
  contenido_unidad: string;
  tipo_precio: string;
  tipo_costo: string;
  stock_minimo: number;
  stock_maximo: number | null;
  garantia_meses: number | null;
  es_materia_prima: boolean;
  ubicacion: string;
  estado: string;
  creado_en: string;
  actualizado_en: string;
}

export interface ItemSchemaRow {
  id: string;
  empresa_id: string | null;
  tipo: TipoItem;
  categoria: string;
  esquema: Record<string, unknown>;
  version: number;
  activo: boolean;
}

const COLUMNAS = `item_id, empresa_id, tipo, codigo, nombre, descripcion,
  categoria, precio_cents, costo_cents, track_inventory, track_serial, stock,
  duration_minutes, atributos, parent_item_id, marca, codigo_barras, unidad,
  permite_fracciones, unidad_compra, contenido_cantidad, contenido_unidad,
  tipo_precio, tipo_costo, stock_minimo, stock_maximo, garantia_meses,
  es_materia_prima, ubicacion, estado, creado_en, actualizado_en`;

/**
 * `catalog.items` expone la PK como `id`; la vista la publica como `item_id`.
 * Un INSERT ... RETURNING debe usar la columna real de la tabla.
 */
const COLUMNAS_RETURNING = COLUMNAS.replace(/^item_id/, 'id AS item_id');

@Injectable()
export class ItemsService {
  private readonly ajv = new Ajv({ allErrors: true, strict: false });

  constructor(private readonly pg: PgService) {}

  async listar(filtros: {
    empresa_id?: unknown;
    tipo?: unknown;
    categoria?: unknown;
    q?: unknown;
    pagina?: unknown;
    limite?: unknown;
    incluir_inactivos?: unknown;
  }): Promise<Pagina<Item>> {
    const empresaId = this.exigirEmpresa(filtros.empresa_id);
    const { pagina, limite } = parsearPaginacion(filtros as Record<string, unknown>);
    const params: unknown[] = [empresaId];
    const condiciones = ['empresa_id = $1'];
    if (filtros.tipo !== undefined) {
      const tipo = this.exigirTipo(filtros.tipo);
      params.push(tipo);
      condiciones.push(`tipo = $${params.length}`);
    }
    if (filtros.categoria) {
      params.push(String(filtros.categoria).trim().toLowerCase());
      condiciones.push(`LOWER(categoria) = $${params.length}`);
    }
    if (filtros.q) {
      params.push(`%${String(filtros.q).trim().toLowerCase()}%`);
      condiciones.push(
        `(LOWER(nombre) LIKE $${params.length} OR LOWER(codigo) LIKE $${params.length})`,
      );
    }
    if (filtros.incluir_inactivos !== true) condiciones.push(`estado = 'activo'`);
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(`SELECT COUNT(*)::int AS n FROM catalog.items_vista ${where}`, params)
      .then((r) => r?.n ?? 0);
    const filas = await this.pg.query<ItemRow>(
      `SELECT ${COLUMNAS} FROM catalog.items_vista ${where}
       ORDER BY creado_en DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, (pagina - 1) * limite],
    );
    const items = filas.map((fila) => this.serializar(fila));
    await this.adjuntarImagenes(items);
    return crearPagina(items, total, { pagina, limite });
  }

  async encontrarPorId(id: string, empresaId?: string): Promise<Item | null> {
    if (!UUID.test(id)) return null;
    const params: unknown[] = [id];
    const condiciones = ['item_id = $1'];
    if (empresaId) {
      params.push(empresaId);
      condiciones.push(`empresa_id = $${params.length}`);
    }
    const fila = await this.pg.queryOne<ItemRow>(
      `SELECT ${COLUMNAS} FROM catalog.items_vista WHERE ${condiciones.join(' AND ')}`,
      params,
    );
    if (!fila) return null;
    const item = this.serializar(fila);
    await this.adjuntarImagenes([item]);
    return item;
  }

  /**
   * Consulta por lote de códigos. Un código puede repetirse en empresas
   * distintas, por eso la fila incluye `empresa_id`: el consumidor debe
   * emparejar la clave compuesta (ADR-27).
   */
  async lotePorSkus(
    skus: string[],
  ): Promise<{
    empresa_id: string;
    sku: string;
    stock: number;
    track_serial: boolean;
    seriales_disponibles: number;
    estado: string;
  }[]> {
    if (!skus.length) return [];
    const params: string[] = [...new Set(skus.map((s) => String(s).toLowerCase()))];
    const marcadores = params.map((_, i) => `$${i + 1}`).join(', ');
    return this.pg.query<{
      empresa_id: string;
      sku: string;
      stock: number;
      track_serial: boolean;
      seriales_disponibles: number;
      estado: string;
    }>(
      `SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.stock, i.track_serial,
              (SELECT COUNT(*)::int FROM catalog.item_series s
                WHERE s.item_id = i.id AND s.estado = 'disponible') AS seriales_disponibles,
              CASE WHEN i.stock > 0
                    AND (NOT i.track_serial OR EXISTS (
                      SELECT 1 FROM catalog.item_series s
                       WHERE s.item_id = i.id AND s.estado = 'disponible'
                    ))
                   THEN 'disponible' ELSE 'agotado' END AS estado
         FROM catalog.items i
        WHERE LOWER(i.codigo) IN (${marcadores})
          AND i.tipo = 'producto' AND i.estado = 'activo' AND i.eliminado_en IS NULL
        ORDER BY i.creado_en DESC`,
      params,
    );
  }

  async crearItem(datos: CrearItemInput, empresaId: string): Promise<Item> {
    const tipo = this.exigirTipo(datos.tipo);
    const codigo = this.normalizarCodigo(datos.codigo);
    const nombre = this.validarNombre(datos.nombre);
    const precio = Money.parsear(datos.precio);
    const costo = Money.parsear(datos.costo ?? '0');
    const trackInventory = tipo === 'producto' ? datos.track_inventory !== false : false;
    if (trackInventory) {
      if (!Number.isInteger(datos.stock) || (datos.stock ?? -1) < 0) {
        throw new DomainError('STOCK_INVALIDO', 'El stock debe ser un entero no negativo.');
      }
    }
    let durationMinutes: number | null = null;
    if (tipo === 'servicio') {
      if (!Number.isInteger(datos.duration_minutes) || (datos.duration_minutes ?? 0) < 1 || (datos.duration_minutes ?? 0) > 1440) {
        throw new DomainError('DURACION_INVALIDA', 'La duración debe estar entre 1 y 1440 minutos.');
      }
      durationMinutes = datos.duration_minutes ?? null;
    }
    if (datos.parent_item_id) {
      const padre = await this.pg.queryOne<{ id: string; tipo: string; empresa_id: string }>(
        `SELECT id, tipo, empresa_id FROM catalog.items WHERE id = $1 AND eliminado_en IS NULL`,
        [datos.parent_item_id],
      );
      if (!padre || padre.empresa_id !== empresaId || padre.tipo !== tipo) {
        throw new DomainError('ITEM_PADRE_INVALIDO', 'El item padre debe existir, ser del mismo tipo y de la misma empresa.');
      }
    }
    const atributos = await this.validarAtributos(
      empresaId,
      tipo,
      datos.categoria ?? 'general',
      datos.atributos ?? {},
    );
    await this.exigirCodigoLibre(empresaId, tipo, codigo);
    let fila: ItemRow | null;
    try {
      fila = await this.pg.queryOne<ItemRow>(
      `INSERT INTO catalog.items
         (empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
          costo_cents, track_inventory, stock, duration_minutes, atributos, estado,
          parent_item_id, marca, codigo_barras, unidad, permite_fracciones,
          unidad_compra, contenido_cantidad, contenido_unidad, tipo_precio,
          tipo_costo, stock_minimo, stock_maximo, garantia_meses, es_materia_prima,
          ubicacion)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::jsonb, $13, $14,
               $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28)
       RETURNING ${COLUMNAS_RETURNING}`,
      [
        empresaId,
        tipo,
        codigo,
        nombre,
        (datos.descripcion ?? '').trim(),
        (datos.categoria ?? 'general').trim() || 'general',
        precio.centavos,
        costo.centavos,
        trackInventory,
        trackInventory ? datos.stock : null,
        durationMinutes,
        JSON.stringify(atributos),
        datos.estado === 'inactivo' ? 'inactivo' : 'activo',
        datos.parent_item_id ?? null,
        (datos.marca ?? '').trim(),
        (datos.codigo_barras ?? '').trim(),
        (datos.unidad ?? 'Unidad (u)').trim() || 'Unidad (u)',
        datos.permite_fracciones === true,
        (datos.unidad_compra ?? 'Igual que la de venta').trim() || 'Igual que la de venta',
        datos.contenido_cantidad ?? 0,
        (datos.contenido_unidad ?? 'Sin contenido').trim() || 'Sin contenido',
        datos.tipo_precio === 'escala' ? 'escala' : 'fijo',
        datos.tipo_costo === 'escala' ? 'escala' : 'fijo',
        datos.stock_minimo ?? 0,
        datos.stock_maximo ?? null,
        datos.garantia_meses ?? null,
        datos.es_materia_prima === true,
        (datos.ubicacion ?? '').trim(),
      ],
      );
    } catch (error) {
      if (esViolacionUnicidad(error)) {
        throw new ConflictError(`Ya existe un item con el código ${codigo} en esta empresa.`);
      }
      throw error;
    }
    if (!fila) throw new DomainError('ITEM_NO_CREADO', 'No se pudo crear el item.');
    return this.serializar(fila);
  }

  /** Un código repetido en la misma empresa y tipo es un 409, no un 500. */
  private async exigirCodigoLibre(empresaId: string, tipo: TipoItem, codigo: string): Promise<void> {
    const existente = await this.pg.queryOne<{ id: string }>(
      `SELECT id FROM catalog.items
        WHERE empresa_id = $1 AND tipo = $2 AND upper(codigo) = upper($3)
          AND eliminado_en IS NULL`,
      [empresaId, tipo, codigo],
    );
    if (existente) {
      throw new ConflictError(`Ya existe un item con el código ${codigo} en esta empresa.`);
    }
  }

  /** Baja lógica genérica de un item (R-DB-5). */
  async eliminar(id: string, empresaId: string): Promise<void> {
    if (!UUID.test(id)) throw new DomainError('ITEM_INVALIDO', 'El id del item no es válido.');
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE catalog.items
          SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL
        RETURNING id`,
      [id, empresaId],
    );
    if (!fila) throw new DomainError('ITEM_NO_ENCONTRADO', 'El item no existe para esta empresa.');
  }

  async crearAlquiler(datos: CrearAlquilerInput, empresaId: string): Promise<Item> {    const codigo = this.normalizarCodigo(datos.codigo);
    const nombre = this.validarNombre(datos.nombre);
    const precio = Money.parsear(datos.precio);
    const costo = Money.parsear(datos.costo ?? '0');
    const atributos = await this.validarAtributos(
      empresaId,
      'alquiler',
      datos.categoria ?? 'general',
      datos.atributos ?? {},
    );
    await this.exigirCodigoLibre(empresaId, 'alquiler', codigo);
    const fila = await this.pg.queryOne<ItemRow>(
      `INSERT INTO catalog.items
         (empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
          costo_cents, track_inventory, stock, atributos, estado)
       VALUES ($1, 'alquiler', $2, $3, $4, $5, $6, $7, false, NULL, $8::jsonb, $9)
       RETURNING ${COLUMNAS_RETURNING}`,
      [
        empresaId,
        codigo,
        nombre,
        (datos.descripcion ?? '').trim(),
        (datos.categoria ?? 'general').trim() || 'general',
        precio.centavos,
        costo.centavos,
        JSON.stringify(atributos),
        datos.estado === 'inactivo' ? 'inactivo' : 'activo',
      ],
    );
    if (!fila) throw new DomainError('ITEM_NO_CREADO', 'No se pudo crear el alquiler.');
    return this.serializar(fila);
  }

  async guardarEsquema(
    empresaId: string | null,
    tipo: unknown,
    categoria: string,
    esquema: Record<string, unknown>,
  ): Promise<void> {
    const itemTipo = this.exigirTipo(tipo);
    const nombreCategoria = categoria.trim().toLowerCase();
    if (!nombreCategoria) throw new DomainError('CATEGORIA_INVALIDA', 'La categoría es obligatoria.');
    if (!esquema || typeof esquema !== 'object' || Array.isArray(esquema)) {
      throw new DomainError('ESQUEMA_INVALIDO', 'El esquema debe ser un objeto JSON Schema.');
    }
    try {
      this.ajv.compile(esquema as JSONSchemaType<Record<string, unknown>>);
    } catch (error) {
      throw new DomainError('ESQUEMA_INVALIDO', 'El esquema JSON no es válido.', {
        mensaje: error instanceof Error ? error.message : 'schema inválido',
      });
    }
    await this.pg.query(
      `INSERT INTO catalog.item_atributo_esquemas
         (empresa_id, tipo, categoria, esquema, version, activo)
       VALUES ($1, $2, $3, $4::jsonb,
               COALESCE((SELECT MAX(version) + 1 FROM catalog.item_atributo_esquemas
                          WHERE empresa_id IS NOT DISTINCT FROM $1 AND tipo = $2
                            AND LOWER(categoria) = LOWER($3)), 1),
               true)
       ON CONFLICT DO NOTHING`,
      [empresaId, itemTipo, nombreCategoria, JSON.stringify(esquema)],
    );
  }

  async esquemas(empresaId?: string): Promise<ItemSchemaRow[]> {
    return this.pg.query<ItemSchemaRow>(
      `SELECT id, empresa_id, tipo, categoria, esquema, version, activo
         FROM catalog.item_atributo_esquemas
        WHERE activo = true AND (empresa_id IS NULL OR empresa_id = $1)
        ORDER BY empresa_id NULLS FIRST, tipo, categoria, version DESC`,
      [empresaId ?? null],
    );
  }

  async validarAtributos(
    empresaId: string,
    tipo: TipoItem,
    categoria: string,
    atributos: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    if (!atributos || typeof atributos !== 'object' || Array.isArray(atributos)) {
      throw new DomainError('ATRIBUTOS_INVALIDOS', 'attributes debe ser un objeto JSON.');
    }
    const esquema = await this.pg.queryOne<{ esquema: Record<string, unknown> }>(
      `SELECT esquema
         FROM catalog.item_atributo_esquemas
        WHERE activo = true AND tipo = $1
          AND LOWER(categoria) IN (LOWER($2), 'general')
          AND (empresa_id = $3 OR empresa_id IS NULL)
        ORDER BY (empresa_id IS NULL), (LOWER(categoria) = LOWER($2)) DESC, version DESC
        LIMIT 1`,
      [tipo, categoria.trim() || 'general', empresaId],
    );
    if (!esquema) return atributos;
    const valido = this.ajv.validate(esquema.esquema, atributos);
    if (!valido) {
      throw new DomainError('ATRIBUTOS_INVALIDOS', 'Los atributos no cumplen el JSON Schema de la categoría.', {
        errores: this.ajv.errors?.map((error: ErrorObject) => ({
          instancia: error.instancePath,
          parametro: error.params,
          mensaje: error.message,
        })),
      });
    }
    return atributos;
  }

  private async adjuntarImagenes(items: Item[]): Promise<void> {
    if (items.length === 0) return;
    const ids = items.map((item) => item.item_id);
    const filas = await this.pg.query<ItemImagen & { item_id: string }>(
      `SELECT id, item_id, objeto_id, orden, es_principal
         FROM catalog.item_imagenes
        WHERE item_id = ANY($1::uuid[])
        ORDER BY item_id, orden`,
      [ids],
    );
    const porItem = new Map<string, ItemImagen[]>();
    for (const fila of filas) {
      const lista = porItem.get(fila.item_id) ?? [];
      lista.push({
        id: fila.id,
        objeto_id: fila.objeto_id,
        orden: fila.orden,
        es_principal: fila.es_principal,
      });
      porItem.set(fila.item_id, lista);
    }
    for (const item of items) item.imagenes = porItem.get(item.item_id) ?? [];
  }

  private serializar(fila: ItemRow): Item {
    return {
      item_id: fila.item_id,
      empresa_id: fila.empresa_id,
      tipo: fila.tipo,
      codigo: fila.codigo,
      nombre: fila.nombre,
      descripcion: fila.descripcion,
      categoria: fila.categoria,
      precio: Money.desdeCentavos(fila.precio_cents).string(),
      costo: Money.desdeCentavos(fila.costo_cents).string(),
      track_inventory: fila.track_inventory,
      track_serial: fila.track_serial === true,
      stock: fila.stock,
      duration_minutes: fila.duration_minutes,
      atributos: fila.atributos ?? {},
      parent_item_id: fila.parent_item_id ?? null,
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
      imagenes: [],
      estado: fila.estado === 'inactivo' ? 'inactivo' : 'activo',
      creado_en: fila.creado_en,
      actualizado_en: fila.actualizado_en,
    };
  }

  private exigirEmpresa(valor: unknown): string {
    const empresaId = typeof valor === 'string' ? valor.trim() : '';
    if (!UUID.test(empresaId)) throw new DomainError('EMPRESA_REQUERIDA', 'Indica una empresa válida.');
    return empresaId;
  }

  private exigirTipo(valor: unknown): TipoItem {
    if (typeof valor !== 'string' || !TIPOS.includes(valor as TipoItem)) {
      throw new DomainError('TIPO_ITEM_INVALIDO', 'El tipo debe ser producto, servicio o alquiler.');
    }
    return valor as TipoItem;
  }

  private normalizarCodigo(valor: string): string {
    const codigo = String(valor).trim().toUpperCase();
    if (!CODIGO.test(codigo)) throw new DomainError('CODIGO_INVALIDO', 'El código solo admite letras, números y guiones (2-32).');
    return codigo;
  }

  private validarNombre(valor: string): string {
    const nombre = String(valor).trim();
    if (nombre.length < 2 || nombre.length > 200) throw new DomainError('NOMBRE_INVALIDO', 'El nombre debe tener entre 2 y 200 caracteres.');
    return nombre;
  }
}
