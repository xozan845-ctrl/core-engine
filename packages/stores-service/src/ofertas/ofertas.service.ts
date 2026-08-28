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
  producto_id: string;
  sku: string;
  producto_nombre: string;
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
  producto_id: string;
  sku: string;
  producto_nombre: string;
  margen: number;
  precio_base_cents: number;
  precio_venta_cents: number;
  stock: number;
  estado: string;
  creado_en: string;
}

interface ProductoExterno {
  id: string;
  sku: string;
  nombre: string;
  precio_base: string;
  stock: number;
  estado: string;
}

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
      `SELECT o.id, o.tienda_id, t.vendedor_id, o.producto_id, o.sku, o.producto_nombre,
              o.margen, o.precio_base_cents, o.precio_venta_cents, o.stock, o.estado, o.creado_en
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
      `SELECT o.id, o.tienda_id, t.vendedor_id, o.producto_id, o.sku, o.producto_nombre,
              o.margen, o.precio_base_cents, o.precio_venta_cents, o.stock, o.estado, o.creado_en
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
         JOIN stores.tiendas t ON t.id = o.tienda_id WHERE t.vendedor_id = $1`,
        [vendedorId],
      )
      .then((r) => r?.n ?? 0);
    const offset = (pagina - 1) * limite;
    const filas = await this.pg.query<OfertaRow>(
      `SELECT o.id, o.tienda_id, t.vendedor_id, o.producto_id, o.sku, o.producto_nombre,
              o.margen, o.precio_base_cents, o.precio_venta_cents, o.stock, o.estado, o.creado_en
       FROM stores.ofertas o
       JOIN stores.tiendas t ON t.id = o.tienda_id
       WHERE t.vendedor_id = $1
       ORDER BY o.creado_en DESC LIMIT $2 OFFSET $3`,
      [vendedorId, limite, offset],
    );
    return crearPagina(filas.map((f) => this.serializar(f)), total, { pagina, limite });
  }

  async deTienda(tiendaId: string): Promise<Oferta[]> {
    const filas = await this.pg.query<OfertaRow>(
      `SELECT o.id, o.tienda_id, t.vendedor_id, o.producto_id, o.sku, o.producto_nombre,
              o.margen, o.precio_base_cents, o.precio_venta_cents, o.stock, o.estado, o.creado_en
       FROM stores.ofertas o
       JOIN stores.tiendas t ON t.id = o.tienda_id
       WHERE o.tienda_id = $1 AND o.estado = 'activa'`,
      [tiendaId],
    );
    return filas.map((f) => this.serializar(f));
  }

  /**
   * H-01 / TC-02: el vendedor publica un producto con su margen (RN-01).
   * RN-02: una oferta no puede publicarse con stock menor a 1.
   */
  async publicar(
    vendedorId: string,
    productoId: string,
    margen: number,
  ): Promise<Oferta> {
    if (margen < MARGEN_MINIMO || margen > MARGEN_MAXIMO) {
      throw new DomainError(
        'MARGEN_INVALIDO',
        `El margen debe estar entre ${MARGEN_MINIMO} y ${MARGEN_MAXIMO} por ciento (RN-01).`,
      );
    }
    const tienda = await this.pg.queryOne<{ id: string }>(
      `SELECT id FROM stores.tiendas WHERE vendedor_id = $1`,
      [vendedorId],
    );
    if (!tienda) {
      throw new DomainError('TIENDA_REQUERIDA', 'Crea primero tu tienda de vendedor.');
    }

    const producto = await this.consultarCatalogo(productoId);
    if (!producto) {
      throw new NotFoundError('Producto', productoId);
    }
    // RN-02
    if (producto.stock < 1 || producto.estado === 'agotado') {
      throw new ConflictError('No se puede publicar un producto con stock menor a 1 (RN-02).');
    }

    const existente = await this.pg.queryOne<{ id: string }>(
      `SELECT id FROM stores.ofertas WHERE tienda_id = $1 AND producto_id = $2`,
      [tienda.id, productoId],
    );
    if (existente) {
      throw new ConflictError('Este producto ya esta publicado en tu tienda.');
    }

    const precioBase = Money.parsear(producto.precio_base);
    const precioVenta = precioBase.aplicarMargen(margen); // RN-01
    const fila = await this.pg.queryOne<OfertaRow>(
      `INSERT INTO stores.ofertas
        (id, tienda_id, producto_id, sku, producto_nombre, margen,
         precio_base_cents, precio_venta_cents, stock, estado)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'activa')
       RETURNING id, tienda_id, producto_id, sku, producto_nombre, margen,
                 precio_base_cents, precio_venta_cents, stock, estado, creado_en`,
      [
        randomUUID(),
        tienda.id,
        productoId,
        producto.sku,
        producto.nombre,
        margen,
        precioBase.centavos,
        precioVenta.centavos,
        producto.stock,
      ],
    );
    if (!fila) throw new DomainError('OFERTA_FALLIDA', 'No se pudo publicar la oferta.');
    await this.pg.query(
      `INSERT INTO stores.historico_precios (oferta_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
       VALUES ($1, NULL, $2, NOW())`,
      [fila.id, precioVenta.centavos],
    );
    return this.serializar(fila);
  }

  /** Cambio de margen del vendedor: afecta solo a compras nuevas. */
  async cambiarMargen(ofertaId: string, vendedorId: string, margen: number): Promise<Oferta> {
    if (margen < MARGEN_MINIMO || margen > MARGEN_MAXIMO) {
      throw new DomainError('MARGEN_INVALIDO', `El margen debe estar entre ${MARGEN_MINIMO} y ${MARGEN_MAXIMO} % (RN-01).`);
    }
    const oferta = await this.pg.queryOne<OfertaRow>(
      `SELECT o.*, t.vendedor_id FROM stores.ofertas o
       JOIN stores.tiendas t ON t.id = o.tienda_id WHERE o.id = $1`,
      [ofertaId],
    );
    if (!oferta) throw new NotFoundError('Oferta', ofertaId);
    if (oferta.vendedor_id !== vendedorId) {
      throw new DomainError('NO_PROPIETARIO', 'Solo el vendedor dueno de la oferta puede editarla.');
    }
    const base = Money.desdeCentavos(oferta.precio_base_cents);
    const nuevo = base.aplicarMargen(margen);
    const actualizado = await this.pg.queryOne<OfertaRow>(
      `UPDATE stores.ofertas SET margen = $2, precio_venta_cents = $3 WHERE id = $1
       RETURNING id, tienda_id, producto_id, sku, producto_nombre, margen,
                 precio_base_cents, precio_venta_cents, stock, estado, creado_en`,
      [ofertaId, margen, nuevo.centavos],
    );
    if (!actualizado) throw new NotFoundError('Oferta', ofertaId);
    await this.pg.query(
      `INSERT INTO stores.historico_precios (oferta_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
       VALUES ($1, $2, $3, NOW())`,
      [ofertaId, oferta.precio_venta_cents, nuevo.centavos],
    );
    return this.serializar(actualizado);
  }

  /** Sincroniza el stock visible de las ofertas con el catalogo (post-reserva). */
  async sincronizarStockDeOferta(sku: string, stockNuevo: number): Promise<void> {
    await this.pg.query(
      `UPDATE stores.ofertas
       SET stock = $2, estado = CASE WHEN $2 > 0 THEN 'activa' ELSE 'agotada' END
       WHERE LOWER(sku) = LOWER($1)`,
      [sku, Math.max(0, stockNuevo)],
    );
  }

  /** Consulta sincrona al catalogo (lecturas se resuelven por REST, sec. 3.2). */
  private async consultarCatalogo(productoId: string): Promise<ProductoExterno | null> {
    try {
      const res = await fetch(`${this.catalogoUrl}/internal/productos/${encodeURIComponent(productoId)}`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) return null;
      return (await res.json()) as ProductoExterno;
    } catch {
      return null;
    }
  }

  private serializar(fila: OfertaRow): Oferta {
    return {
      id: fila.id,
      tienda_id: fila.tienda_id,
      vendedor_id: fila.vendedor_id,
      producto_id: fila.producto_id,
      sku: fila.sku,
      producto_nombre: fila.producto_nombre,
      margen: fila.margen,
      precio_base: Money.desdeCentavos(fila.precio_base_cents).string(),
      precio_venta: Money.desdeCentavos(fila.precio_venta_cents).string(),
      stock: fila.stock,
      estado: fila.estado,
      creado_en: fila.creado_en,
    };
  }
}