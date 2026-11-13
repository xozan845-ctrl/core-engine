import { Injectable, Optional } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  ConflictError,
  DomainError,
  Money,
  NotFoundError,
  Pagina,
  PgService,
  crearPagina,
  parsearPaginacion,
} from '@core/shared';
import { OutboxService, EVENTOS } from '@core/shared';
import { ProductosService } from '../productos/productos.service';
import { ServiciosService } from '../servicios/servicios.service';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const SKU = /^[A-Z0-9-]{2,32}$/;
const MAX_IMPORTACION = 200;
const MAX_LINEAS = 200;

function esViolacionUnicidad(error: unknown): boolean {
  const codigo = (error as { code?: unknown })?.code;
  return codigo === '23505' || codigo === '23P01';
}

export interface ConfiguracionInventario {
  empresa_id: string;
  metodo_costeo: 'promedio' | 'fifo' | 'ueps';
  permitir_stock_negativo: boolean;
  alerta_stock_bajo: boolean;
  actualizar_costo_en_compra: boolean;
  unidad_defecto: string;
  actualizado_en: string;
}

export interface ImportarProductoFila {
  sku: string;
  nombre: string;
  descripcion?: string;
  categoria?: string;
  precio_base: string;
  stock: number;
  costo?: string;
  marca?: string;
  codigo_barras?: string;
  unidad?: string;
  stock_minimo?: number;
  stock_maximo?: number | null;
  es_materia_prima?: boolean;
  ubicacion?: string;
}

export interface ComboComponenteInput {
  item_id?: string;
  sku: string;
  cantidad: number;
}

export interface ComboInput {
  sku: string;
  nombre: string;
  descripcion?: string;
  codigo_barras?: string;
  precio: string | number;
  estado?: 'activo' | 'inactivo';
  componentes: ComboComponenteInput[];
}

export interface ComboComponente {
  item_id: string;
  sku: string;
  nombre: string;
  cantidad: number;
  costo_unitario: string;
  subtotal: string;
}

export interface Combo {
  id: string;
  empresa_id: string;
  sku: string;
  nombre: string;
  descripcion: string;
  codigo_barras: string;
  precio: string;
  costo_total: string;
  margen: number;
  estado: 'activo' | 'inactivo';
  componentes: ComboComponente[];
  creado_en: string;
  actualizado_en: string;
}

export interface PromocionInput {
  nombre: string;
  tipo: '2x1' | 'descuento_cantidad' | 'descuento_porcentaje' | 'regalo';
  item_id?: string;
  sku?: string;
  cantidad_requerida?: number | null;
  cantidad_gratis?: number | null;
  descuento_porcentaje?: number | null;
  fecha_desde: string;
  fecha_hasta: string;
  estado?: 'activo' | 'inactivo';
}

export interface Promocion {
  id: string;
  empresa_id: string;
  nombre: string;
  tipo: string;
  item_id: string | null;
  sku: string;
  cantidad_requerida: number | null;
  cantidad_gratis: number | null;
  descuento_porcentaje: number | null;
  fecha_desde: string;
  fecha_hasta: string;
  estado: 'activo' | 'inactivo';
  creado_en: string;
  actualizado_en: string;
}

export interface MovimientoLineaInput {
  item_id?: string;
  sku: string;
  cantidad: number;
}

export interface MovimientoInput {
  tipo: 'entrada' | 'salida' | 'ajuste' | 'traslado';
  motivo?: string;
  referencia?: string;
  ubicacion_origen?: string;
  ubicacion_destino?: string;
  lineas: MovimientoLineaInput[];
}

export interface Movimiento {
  id: string;
  empresa_id: string;
  tipo: string;
  motivo: string;
  referencia: string;
  ubicacion_origen: string;
  ubicacion_destino: string;
  lineas: Record<string, unknown>[];
  creado_por: string | null;
  creado_en: string;
}

interface ConfiguracionRow {
  empresa_id: string;
  metodo_costeo: string;
  permitir_stock_negativo: boolean;
  alerta_stock_bajo: boolean;
  actualizar_costo_en_compra: boolean;
  unidad_defecto: string;
  actualizado_en: string;
}

interface ComboRow {
  id: string;
  empresa_id: string;
  sku: string;
  nombre: string;
  descripcion: string;
  codigo_barras: string;
  precio_cents: number;
  estado: string;
  componentes: Array<{ item_id: string; sku: string; nombre: string; cantidad: number }>;
  creado_en: string;
  actualizado_en: string;
}

interface PromocionRow {
  id: string;
  empresa_id: string;
  nombre: string;
  tipo: string;
  item_id: string | null;
  sku: string;
  cantidad_requerida: number | null;
  cantidad_gratis: number | null;
  descuento_porcentaje: number | null;
  fecha_desde: string;
  fecha_hasta: string;
  estado: string;
  creado_en: string;
  actualizado_en: string;
}

interface MovimientoRow {
  id: string;
  empresa_id: string;
  tipo: string;
  motivo: string;
  referencia: string;
  ubicacion_origen: string;
  ubicacion_destino: string;
  lineas: Record<string, unknown>[];
  creado_por: string | null;
  creado_en: string;
}

interface ItemResuelto {
  id: string;
  sku: string;
  nombre: string;
  costo_cents: number;
  stock: number;
  es_combo: boolean;
}

/**
 * Operaciones avanzadas del catálogo (R-PS-04). Define el contrato server-side
 * para importación, configuración de inventario, combos, promociones,
 * movimientos/traslados y escaneo por código.
 */
@Injectable()
export class OperacionesService {
  constructor(
    private readonly pg: PgService,
    private readonly productos: ProductosService,
    private readonly servicios: ServiciosService,
    @Optional() private readonly outbox?: OutboxService,
  ) {}

  // ── configuración ────────────────────────────────────────────────────────

  async obtenerConfiguracion(empresaId: string): Promise<ConfiguracionInventario> {
    const fila = await this.pg.queryOne<ConfiguracionRow>(
      `SELECT empresa_id, metodo_costeo, permitir_stock_negativo, alerta_stock_bajo,
              actualizar_costo_en_compra, unidad_defecto, actualizado_en
         FROM catalog.configuracion_inventario WHERE empresa_id = $1`,
      [empresaId],
    );
    if (fila) return this.serializarConfiguracion(fila);
    return {
      empresa_id: empresaId,
      metodo_costeo: 'promedio',
      permitir_stock_negativo: false,
      alerta_stock_bajo: true,
      actualizar_costo_en_compra: true,
      unidad_defecto: 'Unidad (u)',
      actualizado_en: new Date(0).toISOString(),
    };
  }

  async guardarConfiguracion(
    empresaId: string,
    cambios: Partial<Omit<ConfiguracionInventario, 'empresa_id' | 'actualizado_en'>>,
  ): Promise<ConfiguracionInventario> {
    const actual = await this.obtenerConfiguracion(empresaId);
    const metodo = cambios.metodo_costeo ?? actual.metodo_costeo;
    if (!['promedio', 'fifo', 'ueps'].includes(metodo)) {
      throw new DomainError('METODO_COSTEO_INVALIDO', 'El método de costeo no es válido.');
    }
    const fila = await this.pg.queryOne<ConfiguracionRow>(
      `INSERT INTO catalog.configuracion_inventario
         (empresa_id, metodo_costeo, permitir_stock_negativo, alerta_stock_bajo,
          actualizar_costo_en_compra, unidad_defecto)
       VALUES ($1, $2, $3, $4, $5, $6)
       ON CONFLICT (empresa_id) DO UPDATE SET
         metodo_costeo = EXCLUDED.metodo_costeo,
         permitir_stock_negativo = EXCLUDED.permitir_stock_negativo,
         alerta_stock_bajo = EXCLUDED.alerta_stock_bajo,
         actualizar_costo_en_compra = EXCLUDED.actualizar_costo_en_compra,
         unidad_defecto = EXCLUDED.unidad_defecto,
         actualizado_en = NOW()
       RETURNING empresa_id, metodo_costeo, permitir_stock_negativo, alerta_stock_bajo,
                 actualizar_costo_en_compra, unidad_defecto, actualizado_en`,
      [
        empresaId,
        metodo,
        cambios.permitir_stock_negativo ?? actual.permitir_stock_negativo,
        cambios.alerta_stock_bajo ?? actual.alerta_stock_bajo,
        cambios.actualizar_costo_en_compra ?? actual.actualizar_costo_en_compra,
        (cambios.unidad_defecto ?? actual.unidad_defecto).trim() || 'Unidad (u)',
      ],
    );
    if (!fila) throw new DomainError('CONFIGURACION_NO_GUARDADA', 'No se pudo guardar la configuración.');
    return this.serializarConfiguracion(fila);
  }

  // ── importación ──────────────────────────────────────────────────────────

  async importarProductos(
    empresaId: string,
    filas: readonly ImportarProductoFila[],
  ): Promise<{ importados: number; omitidos: number; errores: string[] }> {
    if (filas.length === 0) {
      throw new DomainError('IMPORTACION_VACIA', 'La importación no contiene filas.');
    }
    if (filas.length > MAX_IMPORTACION) {
      throw new DomainError(
        'IMPORTACION_DEMASIADO_GRANDE',
        `El máximo es ${MAX_IMPORTACION} filas por importación.`,
      );
    }
    await this.productos.validarEmpresaCatalogo(empresaId);
    let importados = 0;
    let omitidos = 0;
    const errores: string[] = [];
    for (const [indice, fila] of filas.entries()) {
      try {
        await this.productos.crear(
          {
            empresa_id: empresaId,
            sku: fila.sku,
            nombre: fila.nombre,
            descripcion: fila.descripcion,
            categoria: fila.categoria,
            precio_base: fila.precio_base,
            stock: fila.stock,
            costo: fila.costo,
            marca: fila.marca,
            codigo_barras: fila.codigo_barras,
            unidad: fila.unidad,
            stock_minimo: fila.stock_minimo,
            stock_maximo: fila.stock_maximo,
            es_materia_prima: fila.es_materia_prima,
            ubicacion: fila.ubicacion,
          },
          { empresaValidada: true },
        );
        importados += 1;
      } catch (error) {
        const codigo = error instanceof DomainError ? error.codigo : 'ERROR';
        if (codigo === 'CONFLICTO') omitidos += 1;
        else errores.push(`Fila ${indice + 1}: ${error instanceof Error ? error.message : 'error'}`);
      }
    }
    return { importados, omitidos, errores };
  }

  // ── escaneo ──────────────────────────────────────────────────────────────

  async escanear(
    empresaId: string,
    codigo: string,
  ): Promise<{ tipo: 'producto' | 'servicio'; producto: unknown; servicio: unknown }> {
    const valor = codigo.trim();
    if (!valor) throw new DomainError('CODIGO_REQUERIDO', 'Indica un código para escanear.');
    const producto = await this.productos.encontrarPorCodigoBarras(valor, empresaId);
    if (producto) return { tipo: 'producto', producto, servicio: null };
    const servicio = await this.servicios.encontrarPorCodigo(valor, empresaId);
    if (servicio) return { tipo: 'servicio', producto: null, servicio };
    throw new NotFoundError('Código', valor);
  }

  // ── combos ───────────────────────────────────────────────────────────────

  async listarCombos(
    empresaId: string,
    filtros: { q?: unknown; estado?: unknown; pagina?: unknown; limite?: unknown },
  ): Promise<Pagina<Combo>> {
    const { pagina, limite } = parsearPaginacion(filtros as Record<string, unknown>);
    const params: unknown[] = [empresaId];
    const condiciones: string[] = [
      'i.empresa_id = $1',
      "i.tipo = 'producto'",
      'i.es_combo',
      'i.eliminado_en IS NULL',
    ];
    if (filtros.q) {
      params.push(`%${String(filtros.q).toLowerCase()}%`);
      condiciones.push(
        `(LOWER(i.nombre) LIKE $${params.length} OR LOWER(i.codigo) LIKE $${params.length}
          OR LOWER(i.codigo_barras) LIKE $${params.length})`,
      );
    }
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`i.estado = $${params.length}`);
    }
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(`SELECT COUNT(*)::int AS n FROM catalog.items i ${where}`, params)
      .then((r) => r?.n ?? 0);
    const filas = await this.pg.query<ComboRow>(
      `SELECT i.id, i.empresa_id, i.codigo AS sku, i.nombre, i.descripcion,
              i.codigo_barras, i.precio_cents, i.estado,
              COALESCE(
                jsonb_agg(
                  jsonb_build_object(
                    'item_id', ci.id,
                    'sku', ci.codigo,
                    'nombre', ci.nombre,
                    'cantidad', ic.cantidad
                  )
                  ORDER BY ci.nombre
                ) FILTER (WHERE ic.id IS NOT NULL),
                '[]'::jsonb
              ) AS componentes,
              i.creado_en, i.actualizado_en
         FROM catalog.items i
         LEFT JOIN catalog.item_componentes ic ON ic.item_id = i.id
         LEFT JOIN catalog.items ci
           ON ci.id = ic.componente_item_id AND ci.eliminado_en IS NULL
        ${where}
        GROUP BY i.id
        ORDER BY i.creado_en DESC
        LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, (pagina - 1) * limite],
    );
    return crearPagina(await this.hidratarCombos(filas), total, { pagina, limite });
  }

  async obtenerCombo(id: string, empresaId: string): Promise<Combo | null> {
    if (!UUID.test(id)) return null;
    const fila = await this.pg.queryOne<ComboRow>(
      `SELECT i.id, i.empresa_id, i.codigo AS sku, i.nombre, i.descripcion,
              i.codigo_barras, i.precio_cents, i.estado,
              COALESCE(
                jsonb_agg(
                  jsonb_build_object(
                    'item_id', ci.id,
                    'sku', ci.codigo,
                    'nombre', ci.nombre,
                    'cantidad', ic.cantidad
                  )
                  ORDER BY ci.nombre
                ) FILTER (WHERE ic.id IS NOT NULL),
                '[]'::jsonb
              ) AS componentes,
              i.creado_en, i.actualizado_en
         FROM catalog.items i
         LEFT JOIN catalog.item_componentes ic ON ic.item_id = i.id
         LEFT JOIN catalog.items ci
           ON ci.id = ic.componente_item_id AND ci.eliminado_en IS NULL
        WHERE i.id = $1 AND i.empresa_id = $2
          AND i.tipo = 'producto' AND i.es_combo AND i.eliminado_en IS NULL
        GROUP BY i.id`,
      [id, empresaId],
    );
    if (!fila) return null;
    return (await this.hidratarCombos([fila]))[0];
  }

  async crearCombo(empresaId: string, datos: ComboInput): Promise<Combo> {
    const sku = this.normalizarSku(datos.sku);
    const nombre = this.validarNombre(datos.nombre);
    const precio = Money.parsear(datos.precio);
    const componentes = await this.resolverComponentes(empresaId, datos.componentes);
    const codigoBarras = (datos.codigo_barras ?? '').trim();
    await this.validarComboUnico(empresaId, sku, codigoBarras);
    const id = randomUUID();
    try {
      await this.pg.transaccion(async (client) => {
        await client.query(
          `INSERT INTO catalog.items
             (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
              precio_cents, costo_cents, track_inventory, stock, atributos, estado,
              codigo_barras, es_combo)
           VALUES ($1, $2, 'producto', $3, $4, $5, 'combo', $6, 0, false, NULL,
                   '{}'::jsonb, $7, $8, true)`,
          [
            id,
            empresaId,
            sku,
            nombre,
            (datos.descripcion ?? '').trim(),
            precio.centavos,
            datos.estado === 'inactivo' ? 'inactivo' : 'activo',
            codigoBarras,
          ],
        );
        for (const componente of componentes) {
          await client.query(
            `INSERT INTO catalog.item_componentes
               (empresa_id, item_id, componente_item_id, cantidad)
             VALUES ($1, $2, $3, $4)`,
            [empresaId, id, componente.item_id, componente.cantidad],
          );
        }
      });
    } catch (error) {
      if (esViolacionUnicidad(error)) {
        throw new ConflictError(`Ya existe un item con el SKU ${sku}.`);
      }
      throw error;
    }
    return (await this.obtenerCombo(id, empresaId)) as Combo;
  }

  async actualizarCombo(id: string, empresaId: string, datos: Partial<ComboInput>): Promise<Combo> {
    const actual = await this.obtenerCombo(id, empresaId);
    if (!actual) throw new NotFoundError('Combo', id);
    const componentes =
      datos.componentes === undefined
        ? actual.componentes.map((c) => ({
            item_id: c.item_id,
            sku: c.sku,
            cantidad: c.cantidad,
          }))
        : datos.componentes;
    const resueltos = await this.resolverComponentes(empresaId, componentes);
    const codigoBarras =
      datos.codigo_barras === undefined ? actual.codigo_barras : datos.codigo_barras.trim();
    if (codigoBarras !== actual.codigo_barras) {
      await this.validarComboUnico(empresaId, actual.sku, codigoBarras, id);
    }
    try {
      await this.pg.transaccion(async (client) => {
        const actualizado = await client.query(
          `UPDATE catalog.items
              SET nombre = $3, descripcion = $4, codigo_barras = $5,
                  precio_cents = $6, estado = $7, actualizado_en = NOW()
            WHERE id = $1 AND empresa_id = $2
              AND tipo = 'producto' AND es_combo AND eliminado_en IS NULL`,
          [
            id,
            empresaId,
            datos.nombre === undefined ? actual.nombre : this.validarNombre(datos.nombre),
            datos.descripcion === undefined ? actual.descripcion : datos.descripcion.trim(),
            codigoBarras,
            datos.precio === undefined
              ? Money.parsear(actual.precio).centavos
              : Money.parsear(datos.precio).centavos,
            datos.estado ?? actual.estado,
          ],
        );
        if (actualizado.rowCount === 0) throw new NotFoundError('Combo', id);
        await client.query(
          `DELETE FROM catalog.item_componentes WHERE item_id = $1 AND empresa_id = $2`,
          [id, empresaId],
        );
        for (const componente of resueltos) {
          await client.query(
            `INSERT INTO catalog.item_componentes
               (empresa_id, item_id, componente_item_id, cantidad)
             VALUES ($1, $2, $3, $4)`,
            [empresaId, id, componente.item_id, componente.cantidad],
          );
        }
      });
    } catch (error) {
      if (esViolacionUnicidad(error)) {
        throw new ConflictError(`El código de barras ${codigoBarras} ya está registrado.`);
      }
      throw error;
    }
    return (await this.obtenerCombo(id, empresaId)) as Combo;
  }

  async eliminarCombo(id: string, empresaId: string): Promise<void> {
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE catalog.items SET eliminado_en = NOW(), actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2
          AND tipo = 'producto' AND es_combo AND eliminado_en IS NULL
        RETURNING id`,
      [id, empresaId],
    );
    if (!fila) throw new NotFoundError('Combo', id);
  }

  // ── promociones ──────────────────────────────────────────────────────────

  async listarPromociones(
    empresaId: string,
    filtros: { estado?: unknown; pagina?: unknown; limite?: unknown },
  ): Promise<Pagina<Promocion>> {
    const { pagina, limite } = parsearPaginacion(filtros as Record<string, unknown>);
    const params: unknown[] = [empresaId];
    const condiciones: string[] = ['empresa_id = $1', 'eliminado_en IS NULL'];
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`estado = $${params.length}`);
    }
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(`SELECT COUNT(*)::int AS n FROM catalog.promociones ${where}`, params)
      .then((r) => r?.n ?? 0);
    const filas = await this.pg.query<PromocionRow>(
      `SELECT id, empresa_id, nombre, tipo, item_id, sku, cantidad_requerida,
              cantidad_gratis, descuento_porcentaje, fecha_desde, fecha_hasta,
              estado, creado_en, actualizado_en
         FROM catalog.promociones ${where}
        ORDER BY creado_en DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, (pagina - 1) * limite],
    );
    return crearPagina(filas.map((f) => this.serializarPromocion(f)), total, { pagina, limite });
  }

  async obtenerPromocion(id: string, empresaId: string): Promise<Promocion | null> {
    if (!UUID.test(id)) return null;
    const fila = await this.pg.queryOne<PromocionRow>(
      `SELECT id, empresa_id, nombre, tipo, item_id, sku, cantidad_requerida,
              cantidad_gratis, descuento_porcentaje, fecha_desde, fecha_hasta,
              estado, creado_en, actualizado_en
         FROM catalog.promociones
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL`,
      [id, empresaId],
    );
    return fila ? this.serializarPromocion(fila) : null;
  }

  async crearPromocion(empresaId: string, datos: PromocionInput): Promise<Promocion> {
    const normalizada = await this.normalizarPromocion(empresaId, datos);
    const fila = await this.pg.queryOne<PromocionRow>(
      `INSERT INTO catalog.promociones
         (empresa_id, nombre, tipo, item_id, sku, cantidad_requerida, cantidad_gratis,
          descuento_porcentaje, fecha_desde, fecha_hasta, estado)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
       RETURNING id, empresa_id, nombre, tipo, item_id, sku, cantidad_requerida,
                 cantidad_gratis, descuento_porcentaje, fecha_desde, fecha_hasta,
                 estado, creado_en, actualizado_en`,
      [
        empresaId,
        normalizada.nombre,
        normalizada.tipo,
        normalizada.item_id,
        normalizada.sku,
        normalizada.cantidad_requerida,
        normalizada.cantidad_gratis,
        normalizada.descuento_porcentaje,
        normalizada.fecha_desde,
        normalizada.fecha_hasta,
        normalizada.estado,
      ],
    );
    if (!fila) throw new DomainError('PROMOCION_NO_CREADA', 'No se pudo crear la promoción.');
    return this.serializarPromocion(fila);
  }

  async actualizarPromocion(
    id: string,
    empresaId: string,
    datos: Partial<PromocionInput>,
  ): Promise<Promocion> {
    const actual = await this.obtenerPromocion(id, empresaId);
    if (!actual) throw new NotFoundError('Promoción', id);
    const normalizada = await this.normalizarPromocion(empresaId, {
      nombre: datos.nombre ?? actual.nombre,
      tipo: (datos.tipo ?? actual.tipo) as PromocionInput['tipo'],
      item_id: datos.item_id ?? actual.item_id ?? undefined,
      sku: datos.sku ?? actual.sku,
      cantidad_requerida:
        datos.cantidad_requerida !== undefined ? datos.cantidad_requerida : actual.cantidad_requerida,
      cantidad_gratis:
        datos.cantidad_gratis !== undefined ? datos.cantidad_gratis : actual.cantidad_gratis,
      descuento_porcentaje:
        datos.descuento_porcentaje !== undefined
          ? datos.descuento_porcentaje
          : actual.descuento_porcentaje,
      fecha_desde: datos.fecha_desde ?? actual.fecha_desde.slice(0, 10),
      fecha_hasta: datos.fecha_hasta ?? actual.fecha_hasta.slice(0, 10),
      estado: datos.estado ?? actual.estado,
    });
    const fila = await this.pg.queryOne<PromocionRow>(
      `UPDATE catalog.promociones SET
         nombre = $3, tipo = $4, item_id = $5, sku = $6, cantidad_requerida = $7,
         cantidad_gratis = $8, descuento_porcentaje = $9, fecha_desde = $10,
         fecha_hasta = $11, estado = $12, actualizado_en = NOW()
       WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL
       RETURNING id, empresa_id, nombre, tipo, item_id, sku, cantidad_requerida,
                 cantidad_gratis, descuento_porcentaje, fecha_desde, fecha_hasta,
                 estado, creado_en, actualizado_en`,
      [
        id,
        empresaId,
        normalizada.nombre,
        normalizada.tipo,
        normalizada.item_id,
        normalizada.sku,
        normalizada.cantidad_requerida,
        normalizada.cantidad_gratis,
        normalizada.descuento_porcentaje,
        normalizada.fecha_desde,
        normalizada.fecha_hasta,
        normalizada.estado,
      ],
    );
    if (!fila) throw new NotFoundError('Promoción', id);
    return this.serializarPromocion(fila);
  }

  async eliminarPromocion(id: string, empresaId: string): Promise<void> {
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE catalog.promociones SET eliminado_en = NOW(), actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL RETURNING id`,
      [id, empresaId],
    );
    if (!fila) throw new NotFoundError('Promoción', id);
  }

  // ── movimientos de inventario ────────────────────────────────────────────

  async listarMovimientos(
    empresaId: string,
    filtros: { tipo?: unknown; pagina?: unknown; limite?: unknown },
  ): Promise<Pagina<Movimiento>> {
    const { pagina, limite } = parsearPaginacion(filtros as Record<string, unknown>);
    const params: unknown[] = [empresaId];
    const condiciones: string[] = ['empresa_id = $1'];
    if (filtros.tipo) {
      params.push(String(filtros.tipo));
      condiciones.push(`tipo = $${params.length}`);
    }
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(
        `SELECT COUNT(*)::int AS n FROM catalog.movimientos_inventario ${where}`,
        params,
      )
      .then((r) => r?.n ?? 0);
    const filas = await this.pg.query<MovimientoRow>(
      `SELECT id, empresa_id, tipo, motivo, referencia, ubicacion_origen,
              ubicacion_destino, lineas, creado_por, creado_en
         FROM catalog.movimientos_inventario ${where}
        ORDER BY creado_en DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, (pagina - 1) * limite],
    );
    return crearPagina(filas.map((f) => this.serializarMovimiento(f)), total, { pagina, limite });
  }

  async registrarMovimiento(
    empresaId: string,
    datos: MovimientoInput,
    usuarioId?: string,
  ): Promise<Movimiento> {
    const tipo = datos.tipo;
    if (!['entrada', 'salida', 'ajuste', 'traslado'].includes(tipo)) {
      throw new DomainError('TIPO_MOVIMIENTO_INVALIDO', 'El tipo de movimiento no es válido.');
    }
    if (!Array.isArray(datos.lineas) || datos.lineas.length === 0) {
      throw new DomainError('LINEAS_REQUERIDAS', 'El movimiento requiere al menos una línea.');
    }
    if (datos.lineas.length > MAX_LINEAS) {
      throw new DomainError('DEMASIADAS_LINEAS', `El máximo es ${MAX_LINEAS} líneas.`);
    }
    const origen = (datos.ubicacion_origen ?? '').trim();
    const destino = (datos.ubicacion_destino ?? '').trim();
    if (tipo === 'traslado' && (!origen || !destino || origen === destino)) {
      throw new DomainError(
        'TRASLADO_INVALIDO',
        'Un traslado requiere ubicación de origen y destino distintas.',
      );
    }
    const config = await this.obtenerConfiguracion(empresaId);
    const productos = await this.resolverProductos(empresaId, datos.lineas);
    return this.pg.transaccion(async (client) => {
      const lineas: Record<string, unknown>[] = [];
      const stockItems: { empresa_id: string; sku: string; stock_restante: number }[] = [];
      for (const [indice, linea] of datos.lineas.entries()) {
        const producto = productos[indice];
        const cantidad = Number(linea.cantidad);
        if (!Number.isInteger(cantidad) || cantidad < 0) {
          throw new DomainError('CANTIDAD_INVALIDA', 'La cantidad debe ser un entero mayor o igual a 0.');
        }
        const anterior = producto.stock;
        let nuevo = anterior;
        if (tipo === 'entrada') nuevo = anterior + cantidad;
        else if (tipo === 'salida') nuevo = anterior - cantidad;
        else if (tipo === 'ajuste') nuevo = cantidad;
        if (nuevo < 0 && !config.permitir_stock_negativo) {
          throw new DomainError(
            'STOCK_INSUFICIENTE',
            `Stock insuficiente para ${producto.sku} (disponible ${anterior}).`,
          );
        }
        if (tipo !== 'traslado') {
          await client.query(
            `UPDATE catalog.items
                SET stock = $3, actualizado_en = NOW()
              WHERE id = $1 AND empresa_id = $2 AND tipo = 'producto'
                AND eliminado_en IS NULL`,
            [producto.id, empresaId, nuevo],
          );
          await client.query(
            `INSERT INTO catalog.ajustes_stock
               (id, empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
             VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW())`,
            [empresaId, producto.id, anterior, nuevo, datos.motivo ?? `movimiento ${tipo}`],
          );
          stockItems.push({ empresa_id: empresaId, sku: producto.sku, stock_restante: nuevo });
        }
        lineas.push({
          item_id: producto.id,
          sku: producto.sku,
          nombre: producto.nombre,
          cantidad,
          stock_anterior: anterior,
          stock_nuevo: nuevo,
        });
      }
      const fila = await client.query<MovimientoRow>(
        `INSERT INTO catalog.movimientos_inventario
           (empresa_id, tipo, motivo, referencia, ubicacion_origen, ubicacion_destino,
            lineas, creado_por)
         VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8)
         RETURNING id, empresa_id, tipo, motivo, referencia, ubicacion_origen,
                   ubicacion_destino, lineas, creado_por, creado_en`,
        [
          empresaId,
          tipo,
          (datos.motivo ?? '').trim(),
          (datos.referencia ?? '').trim(),
          origen,
          destino,
          JSON.stringify(lineas),
          usuarioId ?? null,
        ],
      );
      if (this.outbox && stockItems.length > 0) {
        await this.outbox.insertarEnTransaccion(client, EVENTOS.STOCK_UPDATED, {
          order_id: `movimiento-${fila.rows[0].id}`,
          tipo:
            tipo === 'entrada' ? 'entrada' : tipo === 'salida' ? 'salida' : 'ajuste',
          items: stockItems,
        });
      }
      return this.serializarMovimiento(fila.rows[0]);
    });
  }

  // ── helpers ──────────────────────────────────────────────────────────────

  private serializarConfiguracion(fila: ConfiguracionRow): ConfiguracionInventario {
    return {
      empresa_id: fila.empresa_id,
      metodo_costeo: fila.metodo_costeo as ConfiguracionInventario['metodo_costeo'],
      permitir_stock_negativo: fila.permitir_stock_negativo,
      alerta_stock_bajo: fila.alerta_stock_bajo,
      actualizar_costo_en_compra: fila.actualizar_costo_en_compra,
      unidad_defecto: fila.unidad_defecto,
      actualizado_en: fila.actualizado_en,
    };
  }

  private serializarPromocion(fila: PromocionRow): Promocion {
    return {
      id: fila.id,
      empresa_id: fila.empresa_id,
      nombre: fila.nombre,
      tipo: fila.tipo,
      item_id: fila.item_id,
      sku: fila.sku,
      cantidad_requerida: fila.cantidad_requerida,
      cantidad_gratis: fila.cantidad_gratis,
      descuento_porcentaje: fila.descuento_porcentaje,
      fecha_desde: String(fila.fecha_desde).slice(0, 10),
      fecha_hasta: String(fila.fecha_hasta).slice(0, 10),
      estado: fila.estado === 'inactivo' ? 'inactivo' : 'activo',
      creado_en: fila.creado_en,
      actualizado_en: fila.actualizado_en,
    };
  }

  private serializarMovimiento(fila: MovimientoRow): Movimiento {
    return {
      id: fila.id,
      empresa_id: fila.empresa_id,
      tipo: fila.tipo,
      motivo: fila.motivo,
      referencia: fila.referencia,
      ubicacion_origen: fila.ubicacion_origen,
      ubicacion_destino: fila.ubicacion_destino,
      lineas: fila.lineas ?? [],
      creado_por: fila.creado_por,
      creado_en: fila.creado_en,
    };
  }

  private async hidratarCombos(filas: readonly ComboRow[]): Promise<Combo[]> {
    const ids = [...new Set(filas.flatMap((f) => (f.componentes ?? []).map((c) => c.item_id)))];
    const items = ids.length
      ? await this.pg.query<{ id: string; costo_cents: number; nombre: string }>(
          `SELECT id, costo_cents, nombre FROM catalog.items
            WHERE id = ANY($1::uuid[]) AND eliminado_en IS NULL`,
          [ids],
        )
      : [];
    const mapa = new Map(items.map((p) => [p.id, p]));
    return filas.map((fila) => {
      const componentes = (fila.componentes ?? []).map((componente) => {
        const item = mapa.get(componente.item_id);
        const costoUnitario = item?.costo_cents ?? 0;
        return {
          item_id: componente.item_id,
          sku: componente.sku,
          nombre: item?.nombre ?? componente.nombre,
          cantidad: componente.cantidad,
          costo_unitario: Money.desdeCentavos(costoUnitario).string(),
          subtotal: Money.desdeCentavos(costoUnitario * componente.cantidad).string(),
        };
      });
      const costoTotal = componentes.reduce(
        (total, componente) => total + Money.parsear(componente.subtotal).centavos,
        0,
      );
      const precio = Money.desdeCentavos(fila.precio_cents);
      return {
        id: fila.id,
        empresa_id: fila.empresa_id,
        sku: fila.sku,
        nombre: fila.nombre,
        descripcion: fila.descripcion,
        codigo_barras: fila.codigo_barras,
        precio: precio.string(),
        costo_total: Money.desdeCentavos(costoTotal).string(),
        margen:
          precio.centavos > 0
            ? Math.round(((precio.centavos - costoTotal) / precio.centavos) * 1000) / 10
            : 0,
        estado: fila.estado === 'inactivo' ? 'inactivo' : 'activo',
        componentes,
        creado_en: fila.creado_en,
        actualizado_en: fila.actualizado_en,
      };
    });
  }

  private async resolverComponentes(
    empresaId: string,
    componentes: readonly ComboComponenteInput[],
  ): Promise<Array<{ item_id: string; sku: string; nombre: string; cantidad: number }>> {
    if (!Array.isArray(componentes) || componentes.length === 0) {
      throw new DomainError('COMBO_SIN_COMPONENTES', 'El combo requiere al menos un componente.');
    }
    const vistos = new Set<string>();
    const resultado: Array<{ item_id: string; sku: string; nombre: string; cantidad: number }> = [];
    for (const componente of componentes) {
      const cantidad = Number(componente.cantidad);
      if (!Number.isInteger(cantidad) || cantidad < 1) {
        throw new DomainError('CANTIDAD_INVALIDA', 'La cantidad del componente debe ser un entero positivo.');
      }
      const item = await this.buscarItem(empresaId, componente.item_id, componente.sku);
      if (item.es_combo) {
        throw new DomainError(
          'COMPONENTE_INVALIDO',
          'Un combo no puede incluir otro combo; usa sus productos base.',
        );
      }
      if (vistos.has(item.id)) {
        throw new DomainError('COMPONENTE_DUPLICADO', 'Un producto no puede repetirse en el combo.');
      }
      vistos.add(item.id);
      resultado.push({
        item_id: item.id,
        sku: item.sku,
        nombre: item.nombre,
        cantidad,
      });
    }
    return resultado;
  }

  private async validarComboUnico(
    empresaId: string,
    sku: string,
    codigoBarras: string,
    excluirId?: string,
  ): Promise<void> {
    const combo = await this.pg.queryOne(
      `SELECT 1 FROM catalog.items
        WHERE empresa_id = $1 AND tipo = 'producto'
          AND upper(codigo) = upper($2) AND eliminado_en IS NULL
          ${excluirId ? 'AND id <> $3' : ''}`,
      excluirId ? [empresaId, sku, excluirId] : [empresaId, sku],
    );
    if (combo) throw new ConflictError(`Ya existe un item con el SKU ${sku}.`);
    if (!codigoBarras) return;
    const barcode = await this.pg.queryOne(
      `SELECT 1 FROM catalog.items
        WHERE empresa_id = $1 AND codigo_barras = $2 AND eliminado_en IS NULL
          ${excluirId ? 'AND id <> $3' : ''}
        LIMIT 1`,
      excluirId ? [empresaId, codigoBarras, excluirId] : [empresaId, codigoBarras],
    );
    if (barcode) throw new ConflictError(`El código de barras ${codigoBarras} ya está registrado.`);
  }

  private async normalizarPromocion(
    empresaId: string,
    datos: PromocionInput,
  ): Promise<{
    nombre: string;
    tipo: string;
    item_id: string | null;
    sku: string;
    cantidad_requerida: number | null;
    cantidad_gratis: number | null;
    descuento_porcentaje: number | null;
    fecha_desde: string;
    fecha_hasta: string;
    estado: string;
  }> {
    const nombre = this.validarNombre(datos.nombre);
    const tipo = datos.tipo;
    if (!['2x1', 'descuento_cantidad', 'descuento_porcentaje', 'regalo'].includes(tipo)) {
      throw new DomainError('TIPO_PROMOCION_INVALIDO', 'El tipo de promoción no es válido.');
    }
    let cantidadRequerida = datos.cantidad_requerida ?? null;
    let cantidadGratis = datos.cantidad_gratis ?? null;
    const descuento = datos.descuento_porcentaje ?? null;
    if (tipo === '2x1') {
      cantidadRequerida = 2;
      cantidadGratis = 1;
    } else if (tipo === 'regalo') {
      if (!cantidadRequerida || cantidadRequerida < 1 || !cantidadGratis || cantidadGratis < 1) {
        throw new DomainError('PROMOCION_INVALIDA', 'La promoción de regalo requiere cantidades.');
      }
    } else if (tipo === 'descuento_cantidad') {
      if (!cantidadRequerida || cantidadRequerida < 1) {
        throw new DomainError('PROMOCION_INVALIDA', 'Indica la cantidad requerida.');
      }
      if (descuento === null || descuento < 1 || descuento > 100) {
        throw new DomainError('PROMOCION_INVALIDA', 'El descuento debe estar entre 1 y 100.');
      }
    } else if (descuento === null || descuento < 1 || descuento > 100) {
      throw new DomainError('PROMOCION_INVALIDA', 'El descuento debe estar entre 1 y 100.');
    }
    const desde = String(datos.fecha_desde).slice(0, 10);
    const hasta = String(datos.fecha_hasta).slice(0, 10);
    if (!desde || !hasta || hasta < desde) {
      throw new DomainError('VIGENCIA_INVALIDA', 'La vigencia de la promoción no es válida.');
    }
    const item = datos.item_id || datos.sku
      ? await this.buscarItem(empresaId, datos.item_id, datos.sku ?? '')
      : null;
    return {
      nombre,
      tipo,
      item_id: item?.id ?? null,
      sku: item?.sku ?? '',
      cantidad_requerida: cantidadRequerida,
      cantidad_gratis: cantidadGratis,
      descuento_porcentaje: descuento,
      fecha_desde: desde,
      fecha_hasta: hasta,
      estado: datos.estado === 'inactivo' ? 'inactivo' : 'activo',
    };
  }

  private async resolverProductos(
    empresaId: string,
    lineas: readonly MovimientoLineaInput[],
  ): Promise<ItemResuelto[]> {
    const resueltos: ItemResuelto[] = [];
    for (const linea of lineas) {
      resueltos.push(await this.buscarItem(empresaId, linea.item_id, linea.sku));
    }
    return resueltos;
  }

  private async buscarItem(
    empresaId: string,
    itemId: string | undefined,
    sku: string,
  ): Promise<ItemResuelto> {
    const fila = itemId
      ? await this.pg.queryOne<ItemResuelto>(
          `SELECT id, codigo AS sku, nombre, costo_cents, stock, es_combo
             FROM catalog.items
            WHERE empresa_id = $1 AND tipo = 'producto' AND id = $2
              AND eliminado_en IS NULL`,
          [empresaId, itemId],
        )
      : await this.pg.queryOne<ItemResuelto>(
          `SELECT id, codigo AS sku, nombre, costo_cents, stock, es_combo
             FROM catalog.items
            WHERE empresa_id = $1 AND tipo = 'producto'
              AND upper(codigo) = upper($2) AND eliminado_en IS NULL`,
          [empresaId, sku],
        );
    if (!fila) throw new NotFoundError('Producto', itemId ?? sku);
    return fila;
  }

  private normalizarSku(valor: string): string {
    const sku = String(valor).trim().toUpperCase();
    if (!SKU.test(sku)) {
      throw new DomainError('SKU_INVALIDO', 'El SKU solo admite letras, números y guiones (2-32).');
    }
    return sku;
  }

  private validarNombre(valor: string): string {
    const nombre = String(valor).trim();
    if (nombre.length < 2 || nombre.length > 200) {
      throw new DomainError('NOMBRE_INVALIDO', 'El nombre debe tener entre 2 y 200 caracteres.');
    }
    return nombre;
  }
}
