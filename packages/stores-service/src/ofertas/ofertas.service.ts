import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  PgService,
  DomainError,
  ConflictError,
  NotFoundError,
  Money,
  MARGEN_MAXIMO,
  MARGEN_MINIMO,
  Pagina,
  crearPagina,
  parsearPaginacion,
} from '@core/shared';

export interface Oferta {
  id: string;
  tienda_id: string;
  vendedor_id: string;
  empresa_id: string;
  participacion_porcentaje: number;
  item_type: 'producto' | 'servicio' | 'alquiler';
  item_id: string;
  sku: string;
  producto_nombre: string;
  atributos: Record<string, unknown>;
  imagen_objeto_id: string | null;
  imagen_url: string | null;
  margen: number;
  precio_base: string;
  precio_venta: string;
  stock: number;
  estado: string;
  creado_en: string;
}

interface OfertaRow {
  id: string;
  tienda_id: string;
  vendedor_id: string;
  empresa_id: string;
  participacion_porcentaje: number;
  item_type: 'producto' | 'servicio' | 'alquiler';
  item_id: string;
  sku: string;
  producto_nombre: string;
  atributos: Record<string, unknown> | null;
  imagen_objeto_id: string | null;
  margen: number;
  precio_base_cents: number;
  precio_venta_cents: number;
  stock: number;
  estado: string;
  creado_en: string;
}

interface ItemExterno {
  item_id: string;
  empresa_id: string;
  tipo: 'producto' | 'servicio' | 'alquiler';
  codigo: string;
  nombre: string;
  precio: string;
  stock: number | null;
  track_inventory: boolean;
  duration_minutes: number | null;
  atributos: Record<string, unknown>;
  estado: 'activo' | 'inactivo';
  source_type: 'producto' | 'servicio' | null;
  source_id: string | null;
}

const COLUMNAS_OFERTA = `o.id, o.tienda_id, t.vendedor_id, t.empresa_id,
  t.participacion_porcentaje, o.item_type, o.item_id, o.sku,
  o.producto_nombre, o.atributos, o.imagen_objeto_id, o.margen,
  o.precio_base_cents, o.precio_venta_cents, o.stock, o.estado, o.creado_en`;

const URL_IMAGEN_PUBLICA = '/api/v2/storage/public/objetos';

/**
 * Ofertas: relacion vendedor-producto con precio final (Tabla 11).
 * RN-01: precio venta = base x (1 + margen); RN-02: no se publica con stock < 1.
 * El catalogo maestro se consulta al catalog-service de forma sincrona (REST).
 */
@Injectable()
export class OfertasService {
  private readonly catalogoUrl = process.env.CATALOG_SERVICE_URL ?? 'http://catalog-service:3002';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';

  constructor(private readonly pg: PgService) {}

  async encontrarPorId(id: string): Promise<Oferta | null> {
    const fila = await this.pg.queryOne<OfertaRow>(
      `SELECT ${COLUMNAS_OFERTA}
       FROM stores.ofertas o
       JOIN stores.tiendas t ON t.id = o.tienda_id
       WHERE o.id = $1`,
      [id],
    );
    return fila ? this.serializar(fila) : null;
  }

  async porIds(ids: string[]): Promise<Oferta[]> {
    if (ids.length === 0) return [];
    const filas = await this.pg.query<OfertaRow>(
      `SELECT ${COLUMNAS_OFERTA}
       FROM stores.ofertas o
       JOIN stores.tiendas t ON t.id = o.tienda_id
       WHERE o.id = ANY($1)`,
      [ids],
    );
    return filas.map((f) => this.serializar(f));
  }

  async deVendedor(
    vendedorId: string,
    paginacion?: Record<string, unknown>,
  ): Promise<Pagina<Oferta>> {
    const { pagina, limite } = parsearPaginacion(paginacion ?? {});
    const total = await this.pg
      .queryOne<{ n: number }>(
        `SELECT COUNT(*)::int AS n FROM stores.ofertas o
         JOIN stores.tiendas t ON t.id = o.tienda_id
         WHERE t.vendedor_id = $1 OR EXISTS (
           SELECT 1 FROM identity.asignaciones_personal ap
           WHERE ap.usuario_id = $1 AND ap.tienda_id = t.id AND ap.estado = 'activa'
         )`,
        [vendedorId],
      )
      .then((r) => r?.n ?? 0);
    const offset = (pagina - 1) * limite;
    const filas = await this.pg.query<OfertaRow>(
      `SELECT ${COLUMNAS_OFERTA}
       FROM stores.ofertas o
       JOIN stores.tiendas t ON t.id = o.tienda_id
       WHERE t.vendedor_id = $1 OR EXISTS (
         SELECT 1 FROM identity.asignaciones_personal ap
         WHERE ap.usuario_id = $1 AND ap.tienda_id = t.id AND ap.estado = 'activa'
       )
       ORDER BY o.creado_en DESC LIMIT $2 OFFSET $3`,
      [vendedorId, limite, offset],
    );
    return crearPagina(filas.map((f) => this.serializar(f)), total, { pagina, limite });
  }

  async deTienda(tiendaId: string): Promise<Oferta[]> {
    const filas = await this.pg.query<OfertaRow>(
      `SELECT ${COLUMNAS_OFERTA}
       FROM stores.ofertas o
       JOIN stores.tiendas t ON t.id = o.tienda_id
       WHERE o.tienda_id = $1 AND o.estado = 'activa'`,
      [tiendaId],
    );
    return filas.map((f) => this.serializar(f));
  }

  /** Publica cualquier item del catálogo unificado (producto, servicio o alquiler). */
  async publicarItem(
    vendedorId: string,
    tiendaId: string,
    itemId: string,
    margen: number,
  ): Promise<Oferta> {
    if (margen < MARGEN_MINIMO || margen > MARGEN_MAXIMO) {
      throw new DomainError('MARGEN_INVALIDO', `El margen debe estar entre ${MARGEN_MINIMO} y ${MARGEN_MAXIMO} por ciento (RN-01).`);
    }
    const tienda = await this.pg.queryOne<{ id: string; empresa_id: string }>(
      `SELECT id, empresa_id FROM stores.tiendas
       WHERE id = $1 AND (propietario_usuario_id = $2 OR EXISTS (
         SELECT 1 FROM identity.asignaciones_personal ap
         WHERE ap.usuario_id = $2 AND ap.tienda_id = stores.tiendas.id AND ap.estado = 'activa'
       ))`,
      [tiendaId, vendedorId],
    );
    if (!tienda) throw new DomainError('TIENDA_REQUERIDA', 'Crea primero tu tienda de vendedor.');

    const item = await this.consultarItem(itemId);
    if (!item) throw new NotFoundError('Item', itemId);
    if (item.empresa_id !== tienda.empresa_id) {
      throw new DomainError('ITEM_OTRA_EMPRESA', 'La tienda solo puede publicar items de su empresa.');
    }
    if (item.estado !== 'activo') throw new ConflictError('No se puede publicar un item inactivo.');
    if (item.track_inventory && (item.stock ?? 0) < 1) {
      throw new ConflictError('No se puede publicar un item sin stock disponible.');
    }
    const existente = await this.pg.queryOne<{ id: string }>(
      `SELECT id FROM stores.ofertas WHERE tienda_id = $1 AND item_type = $2 AND item_id = $3`,
      [tienda.id, item.tipo, item.item_id],
    );
    if (existente) throw new ConflictError('Este item ya esta publicado en tu tienda.');

    const precioBase = Money.parsear(item.precio);
    const precioVenta = precioBase.aplicarMargen(margen);
    const fila = await this.pg.queryOne<{ id: string }>(
      `INSERT INTO stores.ofertas
        (id, tienda_id, item_type, item_id, sku,
         producto_nombre, atributos, imagen_objeto_id, margen, precio_base_cents,
         precio_venta_cents, stock, estado)
       VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, NULL, $8, $9, $10, $11, 'activa')
       RETURNING id`,
      [
        randomUUID(),
        tienda.id,
        item.tipo,
        item.item_id,
        item.codigo,
        item.nombre,
        JSON.stringify(item.atributos ?? {}),
        margen,
        precioBase.centavos,
        precioVenta.centavos,
        item.track_inventory ? item.stock : null,
      ],
    );
    if (!fila) throw new DomainError('OFERTA_FALLIDA', 'No se pudo publicar el item.');
    await this.pg.query(
      `INSERT INTO stores.historico_precios (oferta_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
       VALUES ($1, NULL, $2, NOW())`,
      [fila.id, precioVenta.centavos],
    );
    const creada = await this.encontrarPorId(fila.id);
    if (!creada) throw new DomainError('OFERTA_FALLIDA', 'No se pudo leer la oferta creada.');
    return creada;
  }

  /** Cambio de margen del vendedor: afecta solo a compras nuevas. */
  async cambiarMargen(ofertaId: string, vendedorId: string, margen: number): Promise<Oferta> {
    if (margen < MARGEN_MINIMO || margen > MARGEN_MAXIMO) {
      throw new DomainError('MARGEN_INVALIDO', `El margen debe estar entre ${MARGEN_MINIMO} y ${MARGEN_MAXIMO} % (RN-01).`);
    }
    const oferta = await this.pg.queryOne<OfertaRow>(
      `SELECT o.*, t.vendedor_id FROM stores.ofertas o
       JOIN stores.tiendas t ON t.id = o.tienda_id
       WHERE o.id = $1 AND (t.vendedor_id = $2 OR EXISTS (
         SELECT 1 FROM identity.asignaciones_personal ap
         WHERE ap.usuario_id = $2 AND ap.tienda_id = t.id AND ap.estado = 'activa'
       ))`,
      [ofertaId, vendedorId],
    );
    if (!oferta) throw new NotFoundError('Oferta', ofertaId);
    const base = Money.desdeCentavos(oferta.precio_base_cents);
    const nuevo = base.aplicarMargen(margen);
    const actualizado = await this.pg.queryOne<{ id: string }>(
      `UPDATE stores.ofertas SET margen = $2, precio_venta_cents = $3 WHERE id = $1
       RETURNING id`,
      [ofertaId, margen, nuevo.centavos],
    );
    if (!actualizado) throw new NotFoundError('Oferta', ofertaId);
    await this.pg.query(
      `INSERT INTO stores.historico_precios (oferta_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
       VALUES ($1, $2, $3, NOW())`,
      [ofertaId, oferta.precio_venta_cents, nuevo.centavos],
    );
    return this.serializar({
      ...oferta,
      margen,
      precio_venta_cents: nuevo.centavos,
    });
  }

  /** Sincroniza el stock visible de las ofertas de UNA empresa (ADR-27). */
  /**
   * Ofertas activas de un producto/variante. Lo usa catalog para impedir que
   * una variante se cree o elimine mientras una oferta sigue viva (ADR-28).
   */
  async contarActivasPorProducto(productoId: string): Promise<number> {
    const fila = await this.pg.queryOne<{ n: number }>(
      `SELECT COUNT(*)::int AS n FROM stores.ofertas
        WHERE item_id = $1 AND estado = 'activa'`,
      [productoId],
    );
    return fila?.n ?? 0;
  }

  async sincronizarStockDeOferta(empresaId: string, sku: string, stockNuevo: number): Promise<void> {
    await this.pg.query(
      `UPDATE stores.ofertas o
       SET stock = $3, estado = CASE WHEN $3 > 0 THEN 'activa' ELSE 'agotada' END
       FROM stores.tiendas t
       WHERE t.id = o.tienda_id
         AND t.empresa_id = $1
         AND o.item_type = 'producto'
         AND LOWER(o.sku) = LOWER($2)`,
      [empresaId, sku, Math.max(0, stockNuevo)],
    );
  }

  private async consultarItem(itemId: string): Promise<ItemExterno | null> {
    try {
      const res = await fetch(`${this.catalogoUrl}/internal/items/${encodeURIComponent(itemId)}`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) return null;
      return (await res.json()) as ItemExterno;
    } catch {
      return null;
    }
  }

  private serializar(fila: OfertaRow): Oferta {
    return {
      id: fila.id,
      tienda_id: fila.tienda_id,
      vendedor_id: fila.vendedor_id,
      empresa_id: fila.empresa_id,
      participacion_porcentaje: fila.participacion_porcentaje,
      item_type: fila.item_type,
      item_id: fila.item_id,
      sku: fila.sku,
      producto_nombre: fila.producto_nombre,
      atributos: fila.atributos ?? {},
      imagen_objeto_id: fila.imagen_objeto_id ?? null,
      imagen_url: fila.imagen_objeto_id
        ? `${URL_IMAGEN_PUBLICA}/${fila.imagen_objeto_id}`
        : null,
      margen: fila.margen,
      precio_base: Money.desdeCentavos(fila.precio_base_cents).string(),
      precio_venta: Money.desdeCentavos(fila.precio_venta_cents).string(),
      stock: fila.stock,
      estado: fila.estado,
      creado_en: fila.creado_en,
    };
  }
}
