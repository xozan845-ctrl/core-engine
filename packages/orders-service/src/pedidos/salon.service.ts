import { Injectable } from '@nestjs/common';
import { ConflictError, DomainError, NotFoundError, PgService } from '@core/shared';

/**
 * Mesas y comandas (Fase P2.5).
 *
 * `mesas` modela el salon y `comandas` los tickets de preparacion por estacion
 * (cocina, bar, etc.). Una comanda puede nacer de una orden o de items libres.
 */

export type EstadoMesa = 'libre' | 'ocupada' | 'reservada' | 'inactiva';
export type EstadoComanda = 'pendiente' | 'en_preparacion' | 'lista' | 'entregada' | 'cancelada';
export const ESTADOS_MESA: EstadoMesa[] = ['libre', 'ocupada', 'reservada', 'inactiva'];
export const ESTADOS_COMANDA: EstadoComanda[] = [
  'pendiente',
  'en_preparacion',
  'lista',
  'entregada',
  'cancelada',
];

export interface Mesa {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  capacidad: number;
  zona: string;
  estado: EstadoMesa;
  creado_en: string;
  actualizado_en: string;
}

export interface ComandaItem {
  sku: string;
  nombre: string;
  cantidad: number;
  notas?: string;
}

export interface Comanda {
  id: string;
  empresa_id: string;
  order_id: string | null;
  mesa_id: string | null;
  estacion: string;
  estado: EstadoComanda;
  notas: string;
  items: ComandaItem[];
  creado_en: string;
  actualizado_en: string;
  listo_en: string | null;
  entregado_en: string | null;
}

export interface CrearMesaInput {
  codigo: string;
  nombre: string;
  capacidad?: number;
  zona?: string;
  estado?: EstadoMesa;
}

export type ActualizarMesaInput = Partial<CrearMesaInput>;

export interface CrearComandaInput {
  order_id?: string | null;
  mesa_id?: string | null;
  estacion: string;
  notas?: string;
  items?: ComandaItem[];
}

export interface ActualizarComandaInput {
  estado?: EstadoComanda;
  notas?: string;
}

interface MesaRow {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  capacidad: number;
  zona: string;
  estado: EstadoMesa;
  creado_en: string;
  actualizado_en: string;
}

interface ComandaRow {
  id: string;
  empresa_id: string;
  order_id: string | null;
  mesa_id: string | null;
  estacion: string;
  estado: EstadoComanda;
  notas: string;
  items_json: ComandaItem[];
  creado_en: string;
  actualizado_en: string;
  listo_en: string | null;
  entregado_en: string | null;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const CODIGO = /^[A-Za-z0-9_-]{1,20}$/;

@Injectable()
export class SalonService {
  constructor(private readonly pg: PgService) {}

  // ── mesas ───────────────────────────────────────────────────────────────

  async listarMesas(
    empresaId: string,
    filtros: { zona?: unknown; estado?: unknown } = {},
  ): Promise<Mesa[]> {
    const params: unknown[] = [empresaId];
    const condiciones = ['empresa_id = $1'];
    if (filtros.zona) {
      params.push(String(filtros.zona));
      condiciones.push(`zona = $${params.length}`);
    }
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`estado = $${params.length}`);
    }
    const filas = await this.pg.query<MesaRow>(
      `SELECT id, empresa_id, codigo, nombre, capacidad, zona, estado,
              creado_en, actualizado_en
         FROM orders.mesas
        WHERE ${condiciones.join(' AND ')}
        ORDER BY zona, codigo`,
      params,
    );
    return filas.map((fila) => this.serializarMesa(fila));
  }

  async crearMesa(empresaId: string, input: CrearMesaInput): Promise<Mesa> {
    const codigo = this.validarCodigo(input.codigo);
    const nombre = this.validarNombre(input.nombre);
    const capacidad = this.validarCapacidad(input.capacidad);
    const zona = String(input.zona ?? '').trim().slice(0, 80);
    const estado = this.validarEstadoMesa(input.estado ?? 'libre');
    try {
      const fila = await this.pg.queryOne<MesaRow>(
        `INSERT INTO orders.mesas
           (empresa_id, codigo, nombre, capacidad, zona, estado)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, empresa_id, codigo, nombre, capacidad, zona, estado,
                   creado_en, actualizado_en`,
        [empresaId, codigo, nombre, capacidad, zona, estado],
      );
      if (!fila) throw new DomainError('MESA_NO_CREADA', 'No se pudo crear la mesa.');
      return this.serializarMesa(fila);
    } catch (error) {
      if (this.esConflicto(error)) {
        throw new ConflictError(`Ya existe una mesa con el codigo ${codigo}.`);
      }
      throw error;
    }
  }

  async actualizarMesa(
    empresaId: string,
    mesaId: string,
    input: ActualizarMesaInput,
  ): Promise<Mesa> {
    const actual = await this.buscarMesa(empresaId, mesaId);
    const codigo = input.codigo === undefined ? actual.codigo : this.validarCodigo(input.codigo);
    const nombre = input.nombre === undefined ? actual.nombre : this.validarNombre(input.nombre);
    const capacidad = input.capacidad === undefined
      ? actual.capacidad
      : this.validarCapacidad(input.capacidad);
    const zona = input.zona === undefined ? actual.zona : String(input.zona).trim().slice(0, 80);
    const estado = input.estado === undefined ? actual.estado : this.validarEstadoMesa(input.estado);
    try {
      const fila = await this.pg.queryOne<MesaRow>(
        `UPDATE orders.mesas
            SET codigo = $3, nombre = $4, capacidad = $5, zona = $6,
                estado = $7, actualizado_en = NOW()
          WHERE id = $1 AND empresa_id = $2
          RETURNING id, empresa_id, codigo, nombre, capacidad, zona, estado,
                    creado_en, actualizado_en`,
        [mesaId, empresaId, codigo, nombre, capacidad, zona, estado],
      );
      if (!fila) throw new NotFoundError('Mesa', mesaId);
      return this.serializarMesa(fila);
    } catch (error) {
      if (this.esConflicto(error)) {
        throw new ConflictError(`Ya existe una mesa con el codigo ${codigo}.`);
      }
      throw error;
    }
  }

  // ── comandas ────────────────────────────────────────────────────────────

  async listarComandas(
    empresaId: string,
    filtros: { estado?: unknown; estacion?: unknown; mesa_id?: unknown } = {},
  ): Promise<Comanda[]> {
    const params: unknown[] = [empresaId];
    const condiciones = ['empresa_id = $1'];
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`estado = $${params.length}`);
    }
    if (filtros.estacion) {
      params.push(String(filtros.estacion));
      condiciones.push(`estacion = $${params.length}`);
    }
    if (filtros.mesa_id) {
      params.push(String(filtros.mesa_id));
      condiciones.push(`mesa_id = $${params.length}`);
    }
    const filas = await this.pg.query<ComandaRow>(
      `SELECT id, empresa_id, order_id, mesa_id, estacion, estado, notas,
              items_json, creado_en, actualizado_en, listo_en, entregado_en
         FROM orders.comandas
        WHERE ${condiciones.join(' AND ')}
        ORDER BY creado_en DESC
        LIMIT 200`,
      params,
    );
    return filas.map((fila) => this.serializarComanda(fila));
  }

  async crearComanda(empresaId: string, input: CrearComandaInput): Promise<Comanda> {
    const orderId = this.validarUuidOpcional(input.order_id, 'orden');
    const mesaId = this.validarUuidOpcional(input.mesa_id, 'mesa');
    const estacion = this.validarEstacion(input.estacion);
    const notas = String(input.notas ?? '').trim().slice(0, 500);
    if (!orderId && (!input.items || input.items.length === 0)) {
      throw new DomainError(
        'COMANDA_SIN_ITEMS',
        'La comanda requiere una orden o una lista de items.',
      );
    }
    if (mesaId) await this.buscarMesa(empresaId, mesaId);

    let items = input.items ? this.validarItems(input.items) : [];
    if (orderId) {
      const orden = await this.pg.queryOne<{
        items_json: Array<{
          empresa_id?: string;
          sku?: string;
          producto_nombre?: string;
          cantidad?: number;
        }>;
      }>(
        `SELECT items_json FROM orders.orden_vista WHERE id = $1`,
        [orderId],
      );
      if (!orden) throw new NotFoundError('Orden', orderId);
      const deEmpresa = (orden.items_json ?? []).filter(
        (item) => !item.empresa_id || item.empresa_id === empresaId,
      );
      items = items.length > 0 ? items : this.itemsDesdeOrden(deEmpresa);
      if (items.length === 0) {
        throw new DomainError('COMANDA_SIN_ITEMS', 'La orden no tiene items para preparar.');
      }
    }

    const fila = await this.pg.queryOne<ComandaRow>(
      `INSERT INTO orders.comandas
         (empresa_id, order_id, mesa_id, estacion, notas, items_json)
       VALUES ($1, $2, $3, $4, $5, $6::jsonb)
       RETURNING id, empresa_id, order_id, mesa_id, estacion, estado, notas,
                 items_json, creado_en, actualizado_en, listo_en, entregado_en`,
      [empresaId, orderId, mesaId, estacion, notas, JSON.stringify(items)],
    );
    if (!fila) throw new DomainError('COMANDA_NO_CREADA', 'No se pudo crear la comanda.');
    if (mesaId) {
      await this.pg.query(
        `UPDATE orders.mesas SET estado = 'ocupada', actualizado_en = NOW()
          WHERE id = $1 AND empresa_id = $2 AND estado IN ('libre', 'reservada')`,
        [mesaId, empresaId],
      );
    }
    return this.serializarComanda(fila);
  }

  async actualizarComanda(
    empresaId: string,
    comandaId: string,
    input: ActualizarComandaInput,
  ): Promise<Comanda> {
    const actual = await this.buscarComanda(empresaId, comandaId);
    const estado = input.estado ?? actual.estado;
    this.validarTransicionComanda(actual.estado, estado);
    const notas = input.notas === undefined ? actual.notas : String(input.notas).trim().slice(0, 500);
    const fila = await this.pg.queryOne<ComandaRow>(
      `UPDATE orders.comandas
          SET estado = $3, notas = $4,
              listo_en = CASE WHEN $3 = 'lista' THEN COALESCE(listo_en, NOW()) ELSE listo_en END,
              entregado_en = CASE WHEN $3 = 'entregada' THEN COALESCE(entregado_en, NOW()) ELSE entregado_en END,
              actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2
        RETURNING id, empresa_id, order_id, mesa_id, estacion, estado, notas,
                  items_json, creado_en, actualizado_en, listo_en, entregado_en`,
      [comandaId, empresaId, estado, notas],
    );
    if (!fila) throw new NotFoundError('Comanda', comandaId);
    return this.serializarComanda(fila);
  }

  // ── helpers ─────────────────────────────────────────────────────────────

  private itemsDesdeOrden(
    items: Array<{ sku?: string; producto_nombre?: string; cantidad?: number }>,
  ): ComandaItem[] {
    return items
      .filter((item) => item.sku && Number(item.cantidad) > 0)
      .map((item) => ({
        sku: String(item.sku),
        nombre: String(item.producto_nombre ?? item.sku),
        cantidad: Number(item.cantidad),
      }));
  }

  private validarItems(items: ComandaItem[]): ComandaItem[] {
    if (!Array.isArray(items) || items.length === 0) {
      throw new DomainError('COMANDA_SIN_ITEMS', 'La comanda requiere al menos un item.');
    }
    return items.map((item) => {
      const sku = String(item.sku ?? '').trim();
      const nombre = String(item.nombre ?? '').trim();
      const cantidad = Number(item.cantidad);
      if (!sku || !nombre || !Number.isInteger(cantidad) || cantidad < 1) {
        throw new DomainError('ITEM_COMANDA_INVALIDO', 'Cada item requiere sku, nombre y cantidad.');
      }
      return {
        sku,
        nombre,
        cantidad,
        ...(item.notas ? { notas: String(item.notas).trim().slice(0, 300) } : {}),
      };
    });
  }

  private async buscarMesa(empresaId: string, mesaId: string): Promise<MesaRow> {
    this.validarUuid(mesaId, 'mesa');
    const fila = await this.pg.queryOne<MesaRow>(
      `SELECT id, empresa_id, codigo, nombre, capacidad, zona, estado,
              creado_en, actualizado_en
         FROM orders.mesas
        WHERE id = $1 AND empresa_id = $2`,
      [mesaId, empresaId],
    );
    if (!fila) throw new NotFoundError('Mesa', mesaId);
    return fila;
  }

  private async buscarComanda(empresaId: string, comandaId: string): Promise<ComandaRow> {
    this.validarUuid(comandaId, 'comanda');
    const fila = await this.pg.queryOne<ComandaRow>(
      `SELECT id, empresa_id, order_id, mesa_id, estacion, estado, notas,
              items_json, creado_en, actualizado_en, listo_en, entregado_en
         FROM orders.comandas
        WHERE id = $1 AND empresa_id = $2`,
      [comandaId, empresaId],
    );
    if (!fila) throw new NotFoundError('Comanda', comandaId);
    return fila;
  }

  private validarTransicionComanda(actual: EstadoComanda, nuevo: EstadoComanda): void {
    const permitidas: Record<EstadoComanda, EstadoComanda[]> = {
      pendiente: ['pendiente', 'en_preparacion', 'cancelada'],
      en_preparacion: ['en_preparacion', 'lista', 'cancelada'],
      lista: ['lista', 'entregada', 'cancelada'],
      entregada: [],
      cancelada: [],
    };
    if (!permitidas[actual].includes(nuevo)) {
      throw new ConflictError(`No se puede pasar de ${actual} a ${nuevo}.`);
    }
  }

  private serializarMesa(fila: MesaRow): Mesa {
    return { ...fila };
  }

  private serializarComanda(fila: ComandaRow): Comanda {
    return {
      id: fila.id,
      empresa_id: fila.empresa_id,
      order_id: fila.order_id,
      mesa_id: fila.mesa_id,
      estacion: fila.estacion,
      estado: fila.estado,
      notas: fila.notas,
      items: fila.items_json ?? [],
      creado_en: fila.creado_en,
      actualizado_en: fila.actualizado_en,
      listo_en: fila.listo_en,
      entregado_en: fila.entregado_en,
    };
  }

  private validarCodigo(valor: unknown): string {
    const codigo = String(valor ?? '').trim();
    if (!CODIGO.test(codigo)) {
      throw new DomainError('CODIGO_INVALIDO', 'El codigo de mesa no es valido.');
    }
    return codigo;
  }

  private validarNombre(valor: unknown): string {
    const nombre = String(valor ?? '').trim();
    if (nombre.length < 1 || nombre.length > 120) {
      throw new DomainError('NOMBRE_INVALIDO', 'El nombre debe tener entre 1 y 120 caracteres.');
    }
    return nombre;
  }

  private validarCapacidad(valor: unknown): number {
    const capacidad = valor === undefined ? 1 : Number(valor);
    if (!Number.isInteger(capacidad) || capacidad < 1 || capacidad > 999) {
      throw new DomainError('CAPACIDAD_INVALIDA', 'La capacidad debe estar entre 1 y 999.');
    }
    return capacidad;
  }

  private validarEstadoMesa(valor: unknown): EstadoMesa {
    const estado = String(valor ?? 'libre') as EstadoMesa;
    if (!ESTADOS_MESA.includes(estado)) {
      throw new DomainError('ESTADO_MESA_INVALIDO', 'El estado de la mesa no es valido.');
    }
    return estado;
  }

  private validarEstacion(valor: unknown): string {
    const estacion = String(valor ?? '').trim();
    if (estacion.length < 2 || estacion.length > 80) {
      throw new DomainError('ESTACION_INVALIDA', 'La estacion debe tener entre 2 y 80 caracteres.');
    }
    return estacion;
  }

  private validarUuidOpcional(valor: unknown, campo: string): string | null {
    if (valor === undefined || valor === null || valor === '') return null;
    return this.validarUuid(valor, campo);
  }

  private validarUuid(valor: unknown, campo: string): string {
    const id = String(valor ?? '');
    if (!UUID.test(id)) throw new DomainError('ID_INVALIDO', `El ${campo} debe ser un UUID.`);
    return id;
  }

  private esConflicto(error: unknown): boolean {
    const codigo = (error as { code?: unknown })?.code;
    return codigo === '23505' || codigo === '23P01';
  }
}
