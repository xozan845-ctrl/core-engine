import { Injectable } from '@nestjs/common';
import type { PoolClient } from 'pg';
import {
  CARRITO_EXPIRACION_MS,
  DomainError,
  Money,
  NotFoundError,
  PgService,
} from '@core/shared';

export interface ItemCarrito {
  oferta_id: string;
  cantidad: number;
}

interface OfertaLectura {
  id: string;
  tienda_id: string;
  vendedor_id: string;
  sku: string;
  producto_nombre: string;
  precio_venta: string;
  stock: number;
  estado: string;
}

interface FilaCarrito {
  comprador_id: string;
  items_json: ItemCarrito[] | null;
  total_cents: number;
  actualizado_en: string;
}

export interface ItemCarritoEnriquecido extends ItemCarrito, OfertaLectura {}

export interface CarritoVista {
  items: ItemCarritoEnriquecido[];
  total: string;
  expira_en: string | null;
}

export const CANTIDAD_MAXIMA_POR_LINEA = 99;

/**
 * Carrito del comprador (RN-05): no reserva stock (RN-03 lo descuenta solo en
 * la creacion de la orden) y expira tras 30 minutos de inactividad.
 * Los precios se enriquecen en vivo contra el stores-service (RN-01).
 */
@Injectable()
export class CarritoService {
  private readonly storesUrl = process.env.STORES_SERVICE_URL ?? 'http://stores-service:3003';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';

  constructor(private readonly pg: PgService) {}

  /** RN-05: purga los carritos inactivos (expirados) antes de cada operacion. */
  private async purgarExpirados(): Promise<void> {
    await this.pg.query(
      `DELETE FROM orders.carritos
       WHERE actualizado_en < NOW() - ($1::int * INTERVAL '1 millisecond')`,
      [CARRITO_EXPIRACION_MS],
    );
  }

  async obtener(compradorId: string): Promise<CarritoVista> {
    await this.purgarExpirados();
    const fila = await this.pg.queryOne<FilaCarrito>(
      `SELECT comprador_id, items_json, total_cents, actualizado_en
       FROM orders.carritos WHERE comprador_id = $1`,
      [compradorId],
    );
    if (!fila) return this.vistaVacia();
    return this.enriquecer(fila);
  }

  /** Agrega (o suma cantidad) una oferta al carrito (RN-05: sin reserva de stock). */
  async agregar(compradorId: string, ofertaId: string, cantidad: number): Promise<CarritoVista> {
    await this.purgarExpirados();
    const oferta = await this.consultarOferta(ofertaId);
    if (!oferta) throw new NotFoundError('Oferta', ofertaId);
    if (oferta.estado !== 'activa') {
      throw new DomainError('OFERTA_NO_DISPONIBLE', 'La oferta no esta disponible.');
    }
    const actuales = await this.itemsDe(compradorId);
    const previa = actuales.find((i) => i.oferta_id === ofertaId);
    const cantidadNueva = (previa?.cantidad ?? 0) + cantidad;
    if (cantidadNueva > CANTIDAD_MAXIMA_POR_LINEA) {
      throw new DomainError(
        'CANTIDAD_MAXIMA',
        `Maximo ${CANTIDAD_MAXIMA_POR_LINEA} unidades por producto.`,
      );
    }
    const items = [
      ...actuales.filter((i) => i.oferta_id !== ofertaId),
      { oferta_id: ofertaId, cantidad: cantidadNueva },
    ];
    return this.persistir(compradorId, items);
  }

  async actualizarCantidad(
    compradorId: string,
    ofertaId: string,
    cantidad: number,
  ): Promise<CarritoVista> {
    await this.purgarExpirados();
    const items = await this.itemsDe(compradorId);
    if (!items.some((i) => i.oferta_id === ofertaId)) {
      throw new NotFoundError('Item del carrito', ofertaId);
    }
    if (cantidad <= 0) return this.quitar(compradorId, ofertaId);
    return this.persistir(compradorId, [
      ...items.filter((i) => i.oferta_id !== ofertaId),
      { oferta_id: ofertaId, cantidad },
    ]);
  }

  async quitar(compradorId: string, ofertaId: string): Promise<CarritoVista> {
    await this.purgarExpirados();
    const items = await this.itemsDe(compradorId);
    if (!items.some((i) => i.oferta_id === ofertaId)) {
      throw new NotFoundError('Item del carrito', ofertaId);
    }
    return this.persistir(compradorId, items.filter((i) => i.oferta_id !== ofertaId));
  }

  async vaciar(compradorId: string): Promise<CarritoVista> {
    await this.purgarExpirados();
    await this.pg.query(`DELETE FROM orders.carritos WHERE comprador_id = $1`, [compradorId]);
    return this.vistaVacia();
  }

  /** Lineas actuales del carrito para el checkout (limpia las expiradas). */
  async itemsParaCheckout(compradorId: string): Promise<ItemCarrito[]> {
    await this.purgarExpirados();
    const fila = await this.pg.queryOne<FilaCarrito>(
      `SELECT items_json FROM orders.carritos WHERE comprador_id = $1`,
      [compradorId],
    );
    return fila?.items_json ?? [];
  }

  /** Elimina el carrito dentro de la transaccion de creacion de la orden. */
  vaciarEnTransaccion(client: PoolClient, compradorId: string): Promise<unknown> {
    return client.query(`DELETE FROM orders.carritos WHERE comprador_id = $1`, [compradorId]);
  }

  private async itemsDe(compradorId: string): Promise<ItemCarrito[]> {
    const fila = await this.pg.queryOne<FilaCarrito>(
      `SELECT items_json FROM orders.carritos WHERE comprador_id = $1`,
      [compradorId],
    );
    return fila?.items_json ?? [];
  }

  private async persistir(
    compradorId: string,
    items: ItemCarrito[],
  ): Promise<CarritoVista> {
    const ofertas = await this.consultarOfertas(items.map((i) => i.oferta_id));
    const porId = new Map(ofertas.map((o) => [o.id, o]));
    let total = Money.desdeCentavos(0);
    const itemsValidos: ItemCarrito[] = [];
    for (const item of items) {
      const oferta = porId.get(item.oferta_id);
      if (!oferta) continue;
      total = total.sumar(Money.parsear(oferta.precio_venta).multiplicarPor(item.cantidad));
      itemsValidos.push(item);
    }
    await this.pg.query(
      `INSERT INTO orders.carritos (comprador_id, items_json, total_cents, actualizado_en)
       VALUES ($1, $2, $3, NOW())
       ON CONFLICT (comprador_id)
       DO UPDATE SET items_json = EXCLUDED.items_json,
                     total_cents = EXCLUDED.total_cents,
                     actualizado_en = NOW()`,
      [compradorId, JSON.stringify(itemsValidos), total.centavos],
    );
    return this.obtener(compradorId);
  }

  private async enriquecer(fila: FilaCarrito): Promise<CarritoVista> {
    const items = fila.items_json ?? [];
    if (items.length === 0) return this.vistaVacia();
    const ofertas = await this.consultarOfertas(items.map((i) => i.oferta_id));
    const porId = new Map(ofertas.map((o) => [o.id, o]));
    let total = Money.desdeCentavos(0);
    const enriquecidos: ItemCarritoEnriquecido[] = [];
    for (const item of items) {
      const oferta = porId.get(item.oferta_id);
      if (!oferta) continue;
      total = total.sumar(Money.parsear(oferta.precio_venta).multiplicarPor(item.cantidad));
      enriquecidos.push({ ...item, ...oferta });
    }
    const actualizado = new Date(fila.actualizado_en).getTime();
    return {
      items: enriquecidos,
      total: total.string(),
      expira_en: new Date(actualizado + CARRITO_EXPIRACION_MS).toISOString(),
    };
  }

  private vistaVacia(): CarritoVista {
    return { items: [], total: '0.00', expira_en: null };
  }

  private async consultarOferta(id: string): Promise<OfertaLectura | null> {
    const [oferta] = await this.consultarOfertas([id]);
    return oferta ?? null;
  }

  private async consultarOfertas(ids: string[]): Promise<OfertaLectura[]> {
    if (ids.length === 0) return [];
    try {
      const res = await fetch(
        `${this.storesUrl}/internal/ofertas?ids=${encodeURIComponent([...new Set(ids)].join(','))}`,
        { headers: { 'x-internal-key': this.claveInterna } },
      );
      if (!res.ok) return [];
      return (await res.json()) as OfertaLectura[];
    } catch {
      return [];
    }
  }
}