import { Injectable } from '@nestjs/common';
import { PgService, Money, ItemOrden } from '@core/shared';

/**
 * OrderView: proyeccion de lectura del lado CQRS (Tabla 8).
 * Lee de orders.orden_vista (poblada por trigger desde event store).
 */
export interface OrderView {
  id: string;
  vendedor_id: string;
  comprador_id: string;
  tienda_id: string;
  estado: string;
  total_cents: number;
  comision_cents: number;
  moneda: string;
  items: ItemOrden[];
  direccion_envio: unknown | null;
  creado_en: string;
  actualizado_en: string;
  pagada_en: string | null;
  enviada_en: string | null;
  entregada_en: string | null;
  cancelada_en: string | null;
  devuelta_en: string | null;
}

interface OrderViewRow {
  id: string;
  vendedor_id: string;
  comprador_id: string;
  tienda_id: string;
  estado: string;
  total_cents: number;
  comision_cents: number;
  moneda: string;
  items_json: unknown;
  direccion_envio: unknown | null;
  creado_en: string;
  actualizado_en: string;
  pagada_en: string | null;
  enviada_en: string | null;
  entregada_en: string | null;
  cancelada_en: string | null;
  devuelta_en: string | null;
}

export interface ComisionVista {
  vendedor_id: string;
  periodo: string;
  ordenes: number;
  total_ventas_cents: number;
  total_comision_cents: number;
  actualizado_en: string;
}

@Injectable()
export class OrderViewRepository {
  constructor(private readonly pg: PgService) {}

  async encontrar(id: string): Promise<OrderView | null> {
    if (!id || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) {
      return null;
    }
    const fila = await this.pg.queryOne<OrderViewRow>(
      `SELECT id, vendedor_id, comprador_id, tienda_id, estado, total_cents, comision_cents, moneda,
              items_json, direccion_envio, creado_en, actualizado_en,
              pagada_en, enviada_en, entregada_en, cancelada_en, devuelta_en
       FROM orders.orden_vista WHERE id = $1`,
      [id],
    );
    return fila ? this.serializar(fila) : null;
  }

  async listarDeCliente(clienteId: string, estado?: string): Promise<OrderView[]> {
    const params: unknown[] = [clienteId];
    const extra = estado ? ` AND estado = $2` : '';
    if (estado) params.push(estado);
    const filas = await this.pg.query<OrderViewRow>(
      `SELECT id, vendedor_id, comprador_id, tienda_id, estado, total_cents, comision_cents, moneda,
              items_json, direccion_envio, creado_en, actualizado_en,
              pagada_en, enviada_en, entregada_en, cancelada_en, devuelta_en
       FROM orders.orden_vista WHERE comprador_id = $1${extra}
       ORDER BY creado_en DESC LIMIT 100`,
      params,
    );
    return filas.map((f) => this.serializar(f));
  }

  async listarDeVendedor(vendedorId: string, estado?: string): Promise<OrderView[]> {
    const params: unknown[] = [vendedorId];
    const extra = estado ? ` AND estado = $2` : '';
    if (estado) params.push(estado);
    const filas = await this.pg.query<OrderViewRow>(
      `SELECT id, vendedor_id, comprador_id, tienda_id, estado, total_cents, comision_cents, moneda,
              items_json, direccion_envio, creado_en, actualizado_en,
              pagada_en, enviada_en, entregada_en, cancelada_en, devuelta_en
       FROM orders.orden_vista WHERE vendedor_id = $1${extra}
       ORDER BY creado_en DESC LIMIT 100`,
      params,
    );
    return filas.map((f) => this.serializar(f));
  }

  async listarDeTienda(tiendaId: string, estado?: string, vendedorId?: string): Promise<OrderView[]> {
    const params: unknown[] = [tiendaId];
    let extra = estado ? ` AND estado = $2` : '';
    if (estado) params.push(estado);
    if (vendedorId) {
      extra += ` AND vendedor_id = $${params.length + 1}`;
      params.push(vendedorId);
    }
    const filas = await this.pg.query<OrderViewRow>(
      `SELECT id, vendedor_id, comprador_id, tienda_id, estado, total_cents, comision_cents, moneda,
              items_json, direccion_envio, creado_en, actualizado_en,
              pagada_en, enviada_en, entregada_en, cancelada_en, devuelta_en
       FROM orders.orden_vista WHERE tienda_id = $1${extra}
       ORDER BY creado_en DESC LIMIT 100`,
      params,
    );
    return filas.map((f) => this.serializar(f));
  }

  async listarTodo(filtros: { estado?: string; limite?: number }): Promise<OrderView[]> {
    const params: unknown[] = [];
    let where = '';
    if (filtros.estado) {
      params.push(filtros.estado);
      where = ' WHERE estado = $1';
    }
    params.push(filtros.limite ?? 200);
    const filas = await this.pg.query<OrderViewRow>(
      `SELECT id, vendedor_id, comprador_id, tienda_id, estado, total_cents, comision_cents, moneda,
              items_json, direccion_envio, creado_en, actualizado_en,
              pagada_en, enviada_en, entregada_en, cancelada_en, devuelta_en
       FROM orders.orden_vista${where} ORDER BY creado_en DESC LIMIT $${params.length}`,
      params,
    );
    return filas.map((f) => this.serializar(f));
  }

  async obtenerComisiones(vendedorId: string, periodo?: string): Promise<ComisionVista[]> {
    const params: unknown[] = [vendedorId];
    let where = 'WHERE vendedor_id = $1';
    if (periodo) {
      params.push(periodo);
      where += ' AND periodo = to_date($2, \'YYYY-MM\')';
    }
    const filas = await this.pg.query<{
      vendedor_id: string;
      periodo: string;
      ordenes: number;
      total_ventas_cents: number;
      total_comision_cents: number;
      actualizado_en: string;
    }>(
      `SELECT vendedor_id, periodo::text, ordenes, total_ventas_cents, total_comision_cents, actualizado_en
       FROM orders.comisiones_vista ${where}
       ORDER BY periodo DESC`,
      params,
    );
    return filas.map((f) => ({
      vendedor_id: f.vendedor_id,
      periodo: f.periodo,
      ordenes: f.ordenes,
      total_ventas_cents: f.total_ventas_cents,
      total_comision_cents: f.total_comision_cents,
      actualizado_en: f.actualizado_en,
    }));
  }

  async obtenerTimeline(orderId: string): Promise<{
    id: number;
    tipo: string;
    payload: unknown;
    version: number;
    creado_en: string;
  }[]> {
    const filas = await this.pg.query<{
      id: number;
      tipo: string;
      payload: unknown;
      version: number;
      creado_en: string;
    }>(
      `SELECT id, tipo, payload, version, creado_en
       FROM orders.orden_timeline WHERE order_id = $1 ORDER BY version`,
      [orderId],
    );
    return filas;
  }

  private serializar(fila: OrderViewRow): OrderView {
    return {
      id: fila.id,
      vendedor_id: fila.vendedor_id,
      comprador_id: fila.comprador_id,
      tienda_id: fila.tienda_id,
      estado: fila.estado,
      total_cents: fila.total_cents,
      comision_cents: fila.comision_cents,
      moneda: fila.moneda,
      items: fila.items_json as ItemOrden[],
      direccion_envio: fila.direccion_envio,
      creado_en: fila.creado_en,
      actualizado_en: fila.actualizado_en,
      pagada_en: fila.pagada_en,
      enviada_en: fila.enviada_en,
      entregada_en: fila.entregada_en,
      cancelada_en: fila.cancelada_en,
      devuelta_en: fila.devuelta_en,
    };
  }
}