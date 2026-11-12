import { Injectable } from '@nestjs/common';
import { ConflictError, DomainError, NotFoundError, PgService } from '@core/shared';

/**
 * Listas de precio y escalas por cantidad (Fase P1.1, ADR-0001).
 *
 * Una lista agrupa precios por segmento (mayorista, VIP, convenio) y puede ser
 * la predeterminada de la empresa. `lista_id NULL` es la escala base del item:
 * es la que da contenido real a `items.tipo_precio = 'escala'`.
 *
 * La resolucion es una lectura: no reserva stock ni crea ordenes. Los servicios
 * que cotizan (orders, stores) pueden degradar al precio de la oferta si el
 * catalogo no responde.
 */

export interface ListaPrecio {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  descripcion: string;
  moneda: string;
  segmento_codigo: string;
  predeterminada: boolean;
  activo: boolean;
  creado_en: string;
  actualizado_en: string;
}

export interface PrecioItem {
  id: string;
  empresa_id: string;
  lista_id: string | null;
  item_id: string;
  desde_cantidad: number;
  hasta_cantidad: number | null;
  precio_cents: number;
  creado_en: string;
  actualizado_en: string;
}

export interface CrearListaInput {
  codigo: string;
  nombre: string;
  descripcion?: string;
  moneda?: string;
  segmento_codigo?: string;
  predeterminada?: boolean;
}

export type ActualizarListaInput = Partial<CrearListaInput> & { activo?: boolean };

export interface CrearPrecioInput {
  item_id: string;
  desde_cantidad?: number;
  hasta_cantidad?: number | null;
  precio_cents: number;
}

export type ActualizarPrecioInput = Partial<Omit<CrearPrecioInput, 'item_id'>>;

export interface PrecioResuelto {
  item_id: string;
  cantidad: number;
  lista: Pick<ListaPrecio, 'id' | 'codigo' | 'nombre' | 'segmento_codigo'> | null;
  origen: 'lista' | 'escala' | 'base';
  escala: PrecioItem | null;
  precio_unitario_cents: number;
  precio_total_cents: number;
}

export interface FiltrosPrecio {
  lista_codigo?: string;
  segmento_codigo?: string;
}

interface ItemPrecio {
  id: string;
  precio_cents: number;
  tipo_precio: 'fijo' | 'escala';
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const CODIGO = /^[a-z][a-z0-9_-]{1,39}$/;
const SEGMENTO = /^[a-z][a-z0-9_-]{0,39}$/;
const MONEDA = /^[A-Z]{3}$/;
const MAX_CANTIDAD = 1_000_000;

const COLUMNAS_LISTA = `id, empresa_id, codigo, nombre, descripcion, moneda,
  segmento_codigo, predeterminada, activo, creado_en, actualizado_en`;
const COLUMNAS_PRECIO = `id, empresa_id, lista_id, item_id, desde_cantidad,
  hasta_cantidad, precio_cents, creado_en, actualizado_en`;

function esConflicto(error: unknown): boolean {
  const codigo = (error as { code?: unknown })?.code;
  return codigo === '23P01' || codigo === '23505';
}

@Injectable()
export class PreciosService {
  constructor(private readonly pg: PgService) {}

  // ── listas ─────────────────────────────────────────────────────────────

  async listarListas(empresaId: string): Promise<ListaPrecio[]> {
    return this.pg.query<ListaPrecio>(
      `SELECT ${COLUMNAS_LISTA}
         FROM catalog.listas_precio
        WHERE empresa_id = $1
        ORDER BY predeterminada DESC, nombre`,
      [empresaId],
    );
  }

  async crearLista(empresaId: string, input: CrearListaInput): Promise<ListaPrecio> {
    const codigo = this.validarCodigo(input.codigo, 'codigo');
    const nombre = this.validarNombre(input.nombre);
    const descripcion = String(input.descripcion ?? '').trim().slice(0, 500);
    const moneda = this.validarMoneda(input.moneda);
    const segmento = this.validarSegmento(input.segmento_codigo);
    const predeterminada = input.predeterminada === true;

    try {
      return await this.pg.transaccion(async (client) => {
        if (predeterminada) {
          await client.query(
            `UPDATE catalog.listas_precio SET predeterminada = false, actualizado_en = NOW()
              WHERE empresa_id = $1 AND predeterminada`,
            [empresaId],
          );
        }
        const resultado = await client.query<ListaPrecio>(
          `INSERT INTO catalog.listas_precio
             (empresa_id, codigo, nombre, descripcion, moneda, segmento_codigo, predeterminada)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING ${COLUMNAS_LISTA}`,
          [empresaId, codigo, nombre, descripcion, moneda, segmento, predeterminada],
        );
        return resultado.rows[0];
      });
    } catch (error) {
      if (esConflicto(error)) {
        throw new ConflictError('La lista ya existe o el segmento ya tiene una lista activa.');
      }
      throw error;
    }
  }

  async actualizarLista(
    empresaId: string,
    listaId: string,
    input: ActualizarListaInput,
  ): Promise<ListaPrecio> {
    this.validarUuid(listaId, 'lista');
    const actual = await this.buscarLista(empresaId, listaId);
    const codigo = input.codigo === undefined ? actual.codigo : this.validarCodigo(input.codigo, 'codigo');
    const nombre = input.nombre === undefined ? actual.nombre : this.validarNombre(input.nombre);
    const descripcion = input.descripcion === undefined
      ? actual.descripcion
      : String(input.descripcion).trim().slice(0, 500);
    const moneda = input.moneda === undefined ? actual.moneda : this.validarMoneda(input.moneda);
    const segmento = input.segmento_codigo === undefined
      ? actual.segmento_codigo
      : this.validarSegmento(input.segmento_codigo);
    const activo = input.activo === undefined ? actual.activo : input.activo === true;
    const predeterminada = (input.predeterminada === undefined ? actual.predeterminada : input.predeterminada === true)
      && activo;

    try {
      return await this.pg.transaccion(async (client) => {
        if (predeterminada && !actual.predeterminada) {
          await client.query(
            `UPDATE catalog.listas_precio SET predeterminada = false, actualizado_en = NOW()
              WHERE empresa_id = $1 AND predeterminada AND id <> $2`,
            [empresaId, listaId],
          );
        }
        const resultado = await client.query<ListaPrecio>(
          `UPDATE catalog.listas_precio
              SET codigo = $1, nombre = $2, descripcion = $3, moneda = $4,
                  segmento_codigo = $5, predeterminada = $6, activo = $7,
                  actualizado_en = NOW()
            WHERE id = $8 AND empresa_id = $9
            RETURNING ${COLUMNAS_LISTA}`,
          [codigo, nombre, descripcion, moneda, segmento, predeterminada, activo, listaId, empresaId],
        );
        const fila = resultado.rows[0];
        if (!fila) throw new NotFoundError('Lista de precios', listaId);
        return fila;
      });
    } catch (error) {
      if (esConflicto(error)) {
        throw new ConflictError('La lista ya existe o el segmento ya tiene una lista activa.');
      }
      throw error;
    }
  }

  // ── precios de una lista ───────────────────────────────────────────────

  async listarPrecios(empresaId: string, listaId: string): Promise<PrecioItem[]> {
    await this.buscarLista(empresaId, listaId);
    return this.pg.query<PrecioItem>(
      `SELECT ${COLUMNAS_PRECIO}
         FROM catalog.lista_precio_items
        WHERE empresa_id = $1 AND lista_id = $2
        ORDER BY item_id, desde_cantidad`,
      [empresaId, listaId],
    );
  }

  async crearPrecio(
    empresaId: string,
    listaId: string | null,
    input: CrearPrecioInput,
  ): Promise<PrecioItem> {
    if (listaId) await this.buscarLista(empresaId, listaId);
    const item = await this.verificarItem(empresaId, input.item_id);
    const { desde, hasta, precio } = this.validarRangoCantidad(input);
    try {
      const fila = await this.pg.queryOne<PrecioItem>(
        `INSERT INTO catalog.lista_precio_items
           (empresa_id, lista_id, item_id, desde_cantidad, hasta_cantidad, precio_cents)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING ${COLUMNAS_PRECIO}`,
        [empresaId, listaId, item.id, desde, hasta, precio],
      );
      if (!fila) throw new DomainError('PRECIO_NO_CREADO', 'No se pudo guardar el precio.');
      if (listaId === null) await this.marcarEscala(empresaId, item.id);
      return fila;
    } catch (error) {
      if (esConflicto(error)) {
        throw new ConflictError('La escala se solapa con otra del mismo item.');
      }
      throw error;
    }
  }

  async actualizarPrecio(
    empresaId: string,
    listaId: string | null,
    precioId: string,
    input: ActualizarPrecioInput,
  ): Promise<PrecioItem> {
    this.validarUuid(precioId, 'precio');
    const actual = await this.buscarPrecio(empresaId, listaId, precioId);
    const { desde, hasta, precio } = this.validarRangoCantidad({
      item_id: actual.item_id,
      desde_cantidad: input.desde_cantidad ?? actual.desde_cantidad,
      hasta_cantidad: input.hasta_cantidad === undefined ? actual.hasta_cantidad : input.hasta_cantidad,
      precio_cents: input.precio_cents ?? actual.precio_cents,
    });
    try {
      const fila = await this.pg.queryOne<PrecioItem>(
        `UPDATE catalog.lista_precio_items
            SET desde_cantidad = $1, hasta_cantidad = $2, precio_cents = $3,
                actualizado_en = NOW()
          WHERE id = $4 AND empresa_id = $5 AND lista_id IS NOT DISTINCT FROM $6::uuid
          RETURNING ${COLUMNAS_PRECIO}`,
        [desde, hasta, precio, precioId, empresaId, listaId],
      );
      if (!fila) throw new NotFoundError('Precio', precioId);
      return fila;
    } catch (error) {
      if (esConflicto(error)) {
        throw new ConflictError('La escala se solapa con otra del mismo item.');
      }
      throw error;
    }
  }

  async eliminarPrecio(
    empresaId: string,
    listaId: string | null,
    precioId: string,
  ): Promise<{ ok: true }> {
    const actual = await this.buscarPrecio(empresaId, listaId, precioId);
    const fila = await this.pg.queryOne<{ id: string }>(
      `DELETE FROM catalog.lista_precio_items
        WHERE id = $1 AND empresa_id = $2 AND lista_id IS NOT DISTINCT FROM $3::uuid
        RETURNING id`,
      [precioId, empresaId, listaId],
    );
    if (!fila) throw new NotFoundError('Precio', precioId);
    if (listaId === null) await this.actualizarTipoPrecio(empresaId, actual.item_id);
    return { ok: true };
  }

  // ── escalas base del item ───────────────────────────────────────────────

  async listarEscalas(empresaId: string, itemId: string): Promise<PrecioItem[]> {
    await this.verificarItem(empresaId, itemId);
    return this.pg.query<PrecioItem>(
      `SELECT ${COLUMNAS_PRECIO}
         FROM catalog.lista_precio_items
        WHERE empresa_id = $1 AND item_id = $2 AND lista_id IS NULL
        ORDER BY desde_cantidad`,
      [empresaId, itemId],
    );
  }

  async actualizarEscala(
    empresaId: string,
    itemId: string,
    escalaId: string,
    input: ActualizarPrecioInput,
  ): Promise<PrecioItem> {
    const actual = await this.buscarPrecio(empresaId, null, escalaId);
    if (actual.item_id !== itemId) throw new NotFoundError('Escala', escalaId);
    return this.actualizarPrecio(empresaId, null, escalaId, input);
  }

  async eliminarEscala(
    empresaId: string,
    itemId: string,
    escalaId: string,
  ): Promise<{ ok: true }> {
    const actual = await this.buscarPrecio(empresaId, null, escalaId);
    if (actual.item_id !== itemId) throw new NotFoundError('Escala', escalaId);
    return this.eliminarPrecio(empresaId, null, escalaId);
  }

  // ── resolucion ──────────────────────────────────────────────────────────

  async resolver(
    empresaId: string,
    itemId: string,
    cantidadValor: unknown,
    filtros: FiltrosPrecio = {},
  ): Promise<PrecioResuelto> {
    const cantidad = this.validarCantidad(cantidadValor);
    const item = await this.verificarItem(empresaId, itemId);
    const lista = await this.resolverLista(empresaId, filtros);
    const escala = await this.buscarEscala(empresaId, item.id, lista?.id ?? null, cantidad);
    const origen: PrecioResuelto['origen'] = escala ? (lista ? 'lista' : 'escala') : 'base';
    const precioUnitario = escala?.precio_cents ?? item.precio_cents;
    return {
      item_id: item.id,
      cantidad,
      lista,
      origen,
      escala,
      precio_unitario_cents: precioUnitario,
      precio_total_cents: this.multiplicar(precioUnitario, cantidad),
    };
  }

  private async resolverLista(
    empresaId: string,
    filtros: FiltrosPrecio,
  ): Promise<PrecioResuelto['lista']> {
    if (filtros.lista_codigo) {
      const codigo = this.validarCodigo(filtros.lista_codigo, 'lista_codigo');
      const lista = await this.pg.queryOne<ListaPrecio>(
        `SELECT ${COLUMNAS_LISTA}
           FROM catalog.listas_precio
          WHERE empresa_id = $1 AND lower(codigo) = $2 AND activo`,
        [empresaId, codigo],
      );
      if (!lista) throw new NotFoundError('Lista de precios', codigo);
      return this.resumenLista(lista);
    }
    if (filtros.segmento_codigo) {
      const segmento = this.validarSegmento(filtros.segmento_codigo);
      const lista = await this.pg.queryOne<ListaPrecio>(
        `SELECT ${COLUMNAS_LISTA}
           FROM catalog.listas_precio
          WHERE empresa_id = $1 AND segmento_codigo = $2 AND activo
          ORDER BY predeterminada DESC, creado_en
          LIMIT 1`,
        [empresaId, segmento],
      );
      if (!lista) throw new NotFoundError('Lista de precios del segmento', segmento);
      return this.resumenLista(lista);
    }
    const predeterminada = await this.pg.queryOne<ListaPrecio>(
      `SELECT ${COLUMNAS_LISTA}
         FROM catalog.listas_precio
        WHERE empresa_id = $1 AND predeterminada AND activo
        LIMIT 1`,
      [empresaId],
    );
    return predeterminada ? this.resumenLista(predeterminada) : null;
  }

  private async buscarEscala(
    empresaId: string,
    itemId: string,
    listaId: string | null,
    cantidad: number,
  ): Promise<PrecioItem | null> {
    return this.pg.queryOne<PrecioItem>(
      `SELECT ${COLUMNAS_PRECIO}
         FROM catalog.lista_precio_items
        WHERE empresa_id = $1 AND item_id = $2 AND lista_id IS NOT DISTINCT FROM $3::uuid
          AND desde_cantidad <= $4
          AND (hasta_cantidad IS NULL OR hasta_cantidad >= $4)
        ORDER BY desde_cantidad DESC
        LIMIT 1`,
      [empresaId, itemId, listaId, cantidad],
    );
  }

  // ── helpers ─────────────────────────────────────────────────────────────

  private resumenLista(lista: ListaPrecio): PrecioResuelto['lista'] {
    return {
      id: lista.id,
      codigo: lista.codigo,
      nombre: lista.nombre,
      segmento_codigo: lista.segmento_codigo,
    };
  }

  private async verificarItem(empresaId: string, itemId: string): Promise<ItemPrecio> {
    this.validarUuid(itemId, 'item');
    const item = await this.pg.queryOne<ItemPrecio>(
      `SELECT id, precio_cents, tipo_precio
         FROM catalog.items
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL`,
      [itemId, empresaId],
    );
    if (!item) throw new NotFoundError('Item', itemId);
    return item;
  }

  private async buscarLista(empresaId: string, listaId: string): Promise<ListaPrecio> {
    this.validarUuid(listaId, 'lista');
    const lista = await this.pg.queryOne<ListaPrecio>(
      `SELECT ${COLUMNAS_LISTA}
         FROM catalog.listas_precio
        WHERE id = $1 AND empresa_id = $2`,
      [listaId, empresaId],
    );
    if (!lista) throw new NotFoundError('Lista de precios', listaId);
    return lista;
  }

  private async buscarPrecio(
    empresaId: string,
    listaId: string | null,
    precioId: string,
  ): Promise<PrecioItem> {
    this.validarUuid(precioId, 'precio');
    const precio = await this.pg.queryOne<PrecioItem>(
      `SELECT ${COLUMNAS_PRECIO}
         FROM catalog.lista_precio_items
        WHERE id = $1 AND empresa_id = $2 AND lista_id IS NOT DISTINCT FROM $3::uuid`,
      [precioId, empresaId, listaId],
    );
    if (!precio) throw new NotFoundError('Precio', precioId);
    return precio;
  }

  private async marcarEscala(empresaId: string, itemId: string): Promise<void> {
    await this.pg.query(
      `UPDATE catalog.items
          SET tipo_precio = 'escala', actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2 AND tipo_precio <> 'escala'`,
      [itemId, empresaId],
    );
  }

  private async actualizarTipoPrecio(empresaId: string, itemId: string): Promise<void> {
    await this.pg.query(
      `UPDATE catalog.items i
          SET tipo_precio = CASE WHEN EXISTS (
                SELECT 1 FROM catalog.lista_precio_items p
                 WHERE p.empresa_id = i.empresa_id AND p.item_id = i.id AND p.lista_id IS NULL
              ) THEN 'escala' ELSE 'fijo' END,
              actualizado_en = NOW()
        WHERE i.id = $1 AND i.empresa_id = $2`,
      [itemId, empresaId],
    );
  }

  private validarRangoCantidad(input: CrearPrecioInput): {
    desde: number;
    hasta: number | null;
    precio: number;
  } {
    const desde = input.desde_cantidad === undefined ? 1 : this.validarCantidad(input.desde_cantidad);
    const hasta = input.hasta_cantidad === undefined || input.hasta_cantidad === null
      ? null
      : this.validarCantidad(input.hasta_cantidad);
    if (hasta !== null && hasta < desde) {
      throw new DomainError('ESCALA_INVALIDA', 'La cantidad final no puede ser menor que la inicial.');
    }
    const precio = Number(input.precio_cents);
    if (!Number.isInteger(precio) || precio < 0) {
      throw new DomainError('PRECIO_INVALIDO', 'El precio debe ser un entero no negativo.');
    }
    return { desde, hasta, precio };
  }

  private validarCantidad(valor: unknown): number {
    const cantidad = Number(valor);
    if (!Number.isInteger(cantidad) || cantidad < 1 || cantidad > MAX_CANTIDAD) {
      throw new DomainError(
        'CANTIDAD_INVALIDA',
        `La cantidad debe ser un entero entre 1 y ${MAX_CANTIDAD}.`,
      );
    }
    return cantidad;
  }

  private validarCodigo(valor: unknown, campo: string): string {
    const codigo = String(valor ?? '').trim().toLowerCase();
    if (!CODIGO.test(codigo)) {
      throw new DomainError(
        'CODIGO_INVALIDO',
        `El ${campo} debe empezar con letra y usar minusculas, numeros, guion o guion bajo.`,
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

  private validarMoneda(valor: unknown): string {
    const moneda = String(valor ?? 'NIO').trim().toUpperCase();
    if (!MONEDA.test(moneda)) {
      throw new DomainError('MONEDA_INVALIDA', 'La moneda debe ser un codigo ISO de 3 letras.');
    }
    return moneda;
  }

  private validarSegmento(valor: unknown): string {
    const segmento = String(valor ?? '').trim().toLowerCase();
    if (segmento !== '' && !SEGMENTO.test(segmento)) {
      throw new DomainError(
        'SEGMENTO_INVALIDO',
        'El segmento debe empezar con letra y usar minusculas, numeros, guion o guion bajo.',
      );
    }
    return segmento;
  }

  private validarUuid(valor: unknown, campo: string): string {
    const id = String(valor ?? '');
    if (!UUID.test(id)) throw new DomainError('ID_INVALIDO', `El ${campo} debe ser un UUID.`);
    return id;
  }

  private multiplicar(precio: number, cantidad: number): number {
    const total = precio * cantidad;
    if (!Number.isSafeInteger(total)) {
      throw new DomainError('TOTAL_INVALIDO', 'El total del precio excede el rango permitido.');
    }
    return total;
  }
}
