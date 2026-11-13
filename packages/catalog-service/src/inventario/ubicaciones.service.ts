import { Injectable } from '@nestjs/common';
import { ConflictError, DomainError, NotFoundError, PgService } from '@core/shared';

/**
 * Multi-bodega/sucursal (Fase P1.2, ADR-0001).
 *
 * `catalog.items.stock` es el total de la empresa y `item_stock_ubicacion` su
 * desglose. Al crear la primera ubicacion se asigna alli el stock existente;
 * las transferencias mueven cantidades sin alterar el total. Los cambios
 * directos de `items.stock` (ordenes, ajustes) los distribuye un trigger.
 */

export type TipoUbicacion = 'bodega' | 'sucursal' | 'almacen' | 'otro';
export const TIPOS_UBICACION: TipoUbicacion[] = ['bodega', 'sucursal', 'almacen', 'otro'];

export interface Ubicacion {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  tipo: TipoUbicacion;
  direccion: string;
  predeterminada: boolean;
  activo: boolean;
  creado_en: string;
  actualizado_en: string;
}

export interface StockUbicacion {
  ubicacion_id: string;
  ubicacion_codigo: string;
  ubicacion_nombre: string;
  item_id: string;
  sku: string;
  nombre: string;
  stock: number;
  stock_total: number;
}

export interface StockItemUbicaciones {
  item_id: string;
  sku: string;
  nombre: string;
  stock_total: number;
  total_ubicaciones: number;
  ubicaciones: {
    ubicacion_id: string;
    codigo: string;
    nombre: string;
    stock: number;
  }[];
}

export interface CrearUbicacionInput {
  codigo: string;
  nombre: string;
  tipo?: TipoUbicacion;
  direccion?: string;
  predeterminada?: boolean;
}

export type ActualizarUbicacionInput = Partial<CrearUbicacionInput> & { activo?: boolean };

export interface TrasladoInput {
  item_id: string;
  origen_id: string;
  destino_id: string;
  cantidad: number;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const CODIGO = /^[a-z][a-z0-9_-]{1,39}$/;
const COLUMNAS = `id, empresa_id, codigo, nombre, tipo, direccion, predeterminada,
  activo, creado_en, actualizado_en`;

@Injectable()
export class UbicacionesService {
  constructor(private readonly pg: PgService) {}

  async listar(empresaId: string): Promise<Ubicacion[]> {
    return this.pg.query<Ubicacion>(
      `SELECT ${COLUMNAS}
         FROM catalog.ubicaciones
        WHERE empresa_id = $1
        ORDER BY predeterminada DESC, nombre`,
      [empresaId],
    );
  }

  async crear(empresaId: string, input: CrearUbicacionInput): Promise<Ubicacion> {
    const codigo = this.validarCodigo(input.codigo);
    const nombre = this.validarNombre(input.nombre);
    const tipo = this.validarTipo(input.tipo);
    const direccion = String(input.direccion ?? '').trim().slice(0, 240);
    const predeterminada = input.predeterminada === true;

    try {
      return await this.pg.transaccion(async (client) => {
        if (predeterminada) {
          await client.query(
            `UPDATE catalog.ubicaciones SET predeterminada = false, actualizado_en = NOW()
              WHERE empresa_id = $1 AND predeterminada`,
            [empresaId],
          );
        }
        const creada = await client.query<Ubicacion>(
          `INSERT INTO catalog.ubicaciones
             (empresa_id, codigo, nombre, tipo, direccion, predeterminada)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING ${COLUMNAS}`,
          [empresaId, codigo, nombre, tipo, direccion, predeterminada],
        );
        const ubicacion = creada.rows[0];

        const total = await client.query<{ total: number }>(
          `SELECT COUNT(*)::int AS total FROM catalog.ubicaciones WHERE empresa_id = $1`,
          [empresaId],
        );
        // La primera ubicacion absorbe el stock que ya existia en el item.
        if (total.rows[0]?.total === 1) {
          await client.query(
            `INSERT INTO catalog.item_stock_ubicacion (empresa_id, item_id, ubicacion_id, stock)
             SELECT i.empresa_id, i.id, $2, COALESCE(i.stock, 0)
               FROM catalog.items i
              WHERE i.empresa_id = $1 AND i.tipo = 'producto'
                AND i.track_inventory AND i.eliminado_en IS NULL
             ON CONFLICT (item_id, ubicacion_id) DO NOTHING`,
            [empresaId, ubicacion.id],
          );
        }
        return ubicacion;
      });
    } catch (error) {
      if (this.esConflicto(error)) {
        throw new ConflictError('La ubicacion ya existe o ya hay una predeterminada.');
      }
      throw error;
    }
  }

  async actualizar(
    empresaId: string,
    ubicacionId: string,
    input: ActualizarUbicacionInput,
  ): Promise<Ubicacion> {
    const actual = await this.buscar(empresaId, ubicacionId);
    const codigo = input.codigo === undefined ? actual.codigo : this.validarCodigo(input.codigo);
    const nombre = input.nombre === undefined ? actual.nombre : this.validarNombre(input.nombre);
    const tipo = input.tipo === undefined ? actual.tipo : this.validarTipo(input.tipo);
    const direccion = input.direccion === undefined
      ? actual.direccion
      : String(input.direccion).trim().slice(0, 240);
    const activo = input.activo === undefined ? actual.activo : input.activo === true;
    const predeterminada = (input.predeterminada === undefined
      ? actual.predeterminada
      : input.predeterminada === true) && activo;

    if (!activo && actual.activo) {
      const ocupada = await this.pg.queryOne<{ total: number }>(
        `SELECT COALESCE(SUM(stock), 0)::int AS total
           FROM catalog.item_stock_ubicacion
          WHERE empresa_id = $1 AND ubicacion_id = $2 AND stock > 0`,
        [empresaId, ubicacionId],
      );
      if ((ocupada?.total ?? 0) > 0) {
        throw new ConflictError('No se puede desactivar una ubicacion con stock.');
      }
    }

    try {
      return await this.pg.transaccion(async (client) => {
        if (predeterminada && !actual.predeterminada) {
          await client.query(
            `UPDATE catalog.ubicaciones SET predeterminada = false, actualizado_en = NOW()
              WHERE empresa_id = $1 AND predeterminada AND id <> $2`,
            [empresaId, ubicacionId],
          );
        }
        const fila = await client.query<Ubicacion>(
          `UPDATE catalog.ubicaciones
              SET codigo = $1, nombre = $2, tipo = $3, direccion = $4,
                  predeterminada = $5, activo = $6, actualizado_en = NOW()
            WHERE id = $7 AND empresa_id = $8
            RETURNING ${COLUMNAS}`,
          [codigo, nombre, tipo, direccion, predeterminada, activo, ubicacionId, empresaId],
        );
        if (!fila.rows[0]) throw new NotFoundError('Ubicacion', ubicacionId);
        return fila.rows[0];
      });
    } catch (error) {
      if (this.esConflicto(error)) {
        throw new ConflictError('La ubicacion ya existe o ya hay una predeterminada.');
      }
      throw error;
    }
  }

  async listarStock(empresaId: string, ubicacionId: string): Promise<StockUbicacion[]> {
    await this.buscar(empresaId, ubicacionId);
    return this.pg.query<StockUbicacion>(
      `SELECT s.ubicacion_id, u.codigo AS ubicacion_codigo, u.nombre AS ubicacion_nombre,
              s.item_id, i.codigo AS sku, i.nombre, s.stock, i.stock AS stock_total
         FROM catalog.item_stock_ubicacion s
         JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
         JOIN catalog.items i ON i.id = s.item_id
        WHERE s.empresa_id = $1 AND s.ubicacion_id = $2 AND i.eliminado_en IS NULL
        ORDER BY i.nombre`,
      [empresaId, ubicacionId],
    );
  }

  async stockDeItem(empresaId: string, itemId: string): Promise<StockItemUbicaciones> {
    const item = await this.verificarItem(empresaId, itemId);
    const ubicaciones = await this.pg.query<{
      ubicacion_id: string;
      codigo: string;
      nombre: string;
      stock: number;
    }>(
      `SELECT s.ubicacion_id, u.codigo, u.nombre, s.stock
         FROM catalog.item_stock_ubicacion s
         JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
        WHERE s.empresa_id = $1 AND s.item_id = $2
        ORDER BY u.predeterminada DESC, u.nombre`,
      [empresaId, itemId],
    );
    return {
      item_id: item.id,
      sku: item.codigo,
      nombre: item.nombre,
      stock_total: item.stock,
      total_ubicaciones: ubicaciones.reduce((total, fila) => total + fila.stock, 0),
      ubicaciones,
    };
  }

  async ajustarStock(
    empresaId: string,
    ubicacionId: string,
    itemId: string,
    stockValor: unknown,
  ): Promise<StockUbicacion> {
    await this.buscarActiva(empresaId, ubicacionId);
    const item = await this.verificarItem(empresaId, itemId);
    if (!item.track_inventory) {
      throw new DomainError('INVENTARIO_NO_APLICA', 'El item no controla inventario.');
    }
    const stock = this.validarStock(stockValor);

    return this.pg.transaccion(async (client) => {
      await client.query(
        `INSERT INTO catalog.item_stock_ubicacion AS s
           (empresa_id, item_id, ubicacion_id, stock)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (item_id, ubicacion_id) DO UPDATE
           SET stock = EXCLUDED.stock, actualizado_en = NOW()`,
        [empresaId, itemId, ubicacionId, stock],
      );
      await this.sincronizarTotal(client, empresaId, itemId);
      const fila = await client.query<StockUbicacion>(
        `SELECT s.ubicacion_id, u.codigo AS ubicacion_codigo, u.nombre AS ubicacion_nombre,
                s.item_id, i.codigo AS sku, i.nombre, s.stock, i.stock AS stock_total
           FROM catalog.item_stock_ubicacion s
           JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
           JOIN catalog.items i ON i.id = s.item_id
          WHERE s.empresa_id = $1 AND s.item_id = $2 AND s.ubicacion_id = $3`,
        [empresaId, itemId, ubicacionId],
      );
      return fila.rows[0];
    });
  }

  async trasladar(empresaId: string, input: TrasladoInput): Promise<{
    origen: StockUbicacion;
    destino: StockUbicacion;
  }> {
    const item = await this.verificarItem(empresaId, input.item_id);
    const cantidad = this.validarCantidad(input.cantidad);
    if (input.origen_id === input.destino_id) {
      throw new DomainError('TRASLADO_INVALIDO', 'El origen y el destino deben ser distintos.');
    }
    await this.buscarActiva(empresaId, input.origen_id);
    await this.buscarActiva(empresaId, input.destino_id);

    return this.pg.transaccion(async (client) => {
      const origen = await client.query<{ stock: number }>(
        `SELECT stock FROM catalog.item_stock_ubicacion
          WHERE empresa_id = $1 AND item_id = $2 AND ubicacion_id = $3
          FOR UPDATE`,
        [empresaId, item.id, input.origen_id],
      );
      if (!origen.rows[0] || origen.rows[0].stock < cantidad) {
        throw new ConflictError('La ubicacion de origen no tiene stock suficiente.', {
          disponible: origen.rows[0]?.stock ?? 0,
        }, 'STOCK_UBICACION_INSUFICIENTE');
      }
      await client.query(
        `UPDATE catalog.item_stock_ubicacion
            SET stock = stock - $1, actualizado_en = NOW()
          WHERE empresa_id = $2 AND item_id = $3 AND ubicacion_id = $4`,
        [cantidad, empresaId, item.id, input.origen_id],
      );
      await client.query(
        `INSERT INTO catalog.item_stock_ubicacion AS s
           (empresa_id, item_id, ubicacion_id, stock)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (item_id, ubicacion_id) DO UPDATE
           SET stock = s.stock + EXCLUDED.stock, actualizado_en = NOW()`,
        [empresaId, item.id, input.destino_id, cantidad],
      );
      const filas = await client.query<StockUbicacion>(
        `SELECT s.ubicacion_id, u.codigo AS ubicacion_codigo, u.nombre AS ubicacion_nombre,
                s.item_id, i.codigo AS sku, i.nombre, s.stock, i.stock AS stock_total
           FROM catalog.item_stock_ubicacion s
           JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
           JOIN catalog.items i ON i.id = s.item_id
          WHERE s.empresa_id = $1 AND s.item_id = $2
            AND s.ubicacion_id IN ($3, $4)`,
        [empresaId, item.id, input.origen_id, input.destino_id],
      );
      const porUbicacion = new Map(filas.rows.map((fila) => [fila.ubicacion_id, fila]));
      return {
        origen: porUbicacion.get(input.origen_id) as StockUbicacion,
        destino: porUbicacion.get(input.destino_id) as StockUbicacion,
      };
    });
  }

  // ── helpers ─────────────────────────────────────────────────────────────

  private async sincronizarTotal(
    client: { query: (sql: string, params?: unknown[]) => Promise<unknown> },
    empresaId: string,
    itemId: string,
  ): Promise<void> {
    await client.query(`SELECT set_config('catalog.stock_sync', 'on', true)`);
    await client.query(
      `UPDATE catalog.items
          SET stock = (
                SELECT COALESCE(SUM(stock), 0)::int
                  FROM catalog.item_stock_ubicacion
                 WHERE empresa_id = $1 AND item_id = $2
              ),
              actualizado_en = NOW()
        WHERE id = $2 AND empresa_id = $1`,
      [empresaId, itemId],
    );
    await client.query(`SELECT set_config('catalog.stock_sync', 'off', true)`);
  }

  private async buscar(empresaId: string, ubicacionId: string): Promise<Ubicacion> {
    this.validarUuid(ubicacionId, 'ubicacion');
    const ubicacion = await this.pg.queryOne<Ubicacion>(
      `SELECT ${COLUMNAS}
         FROM catalog.ubicaciones
        WHERE id = $1 AND empresa_id = $2`,
      [ubicacionId, empresaId],
    );
    if (!ubicacion) throw new NotFoundError('Ubicacion', ubicacionId);
    return ubicacion;
  }

  private async buscarActiva(empresaId: string, ubicacionId: string): Promise<Ubicacion> {
    const ubicacion = await this.buscar(empresaId, ubicacionId);
    if (!ubicacion.activo) {
      throw new ConflictError('La ubicacion esta inactiva.', undefined, 'UBICACION_INACTIVA');
    }
    return ubicacion;
  }

  private async verificarItem(
    empresaId: string,
    itemId: string,
  ): Promise<{ id: string; codigo: string; nombre: string; stock: number; track_inventory: boolean }> {
    this.validarUuid(itemId, 'item');
    const item = await this.pg.queryOne<{
      id: string;
      codigo: string;
      nombre: string;
      stock: number;
      track_inventory: boolean;
    }>(
      `SELECT id, codigo, nombre, stock, track_inventory
         FROM catalog.items
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL`,
      [itemId, empresaId],
    );
    if (!item) throw new NotFoundError('Item', itemId);
    return item;
  }

  private validarCodigo(valor: unknown): string {
    const codigo = String(valor ?? '').trim().toLowerCase();
    if (!CODIGO.test(codigo)) {
      throw new DomainError(
        'CODIGO_INVALIDO',
        'El codigo debe empezar con letra y usar minusculas, numeros, guion o guion bajo.',
      );
    }
    return codigo;
  }

  private validarNombre(valor: unknown): string {
    const nombre = String(valor ?? '').trim();
    if (nombre.length < 2 || nombre.length > 120) {
      throw new DomainError('NOMBRE_INVALIDO', 'El nombre debe tener entre 2 y 120 caracteres.');
    }
    return nombre;
  }

  private validarTipo(valor: unknown): TipoUbicacion {
    const tipo = String(valor ?? 'bodega').trim() as TipoUbicacion;
    if (!TIPOS_UBICACION.includes(tipo)) {
      throw new DomainError('TIPO_UBICACION_INVALIDO', 'El tipo de ubicacion no es valido.');
    }
    return tipo;
  }

  private validarStock(valor: unknown): number {
    const stock = Number(valor);
    if (!Number.isInteger(stock) || stock < 0) {
      throw new DomainError('STOCK_INVALIDO', 'El stock debe ser un entero no negativo.');
    }
    return stock;
  }

  private validarCantidad(valor: unknown): number {
    const cantidad = Number(valor);
    if (!Number.isInteger(cantidad) || cantidad < 1) {
      throw new DomainError('CANTIDAD_INVALIDA', 'La cantidad debe ser un entero positivo.');
    }
    return cantidad;
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
