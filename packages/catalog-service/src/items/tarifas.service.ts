import { Injectable } from '@nestjs/common';
import { ConflictError, DomainError, NotFoundError, PgService } from '@core/shared';

/**
 * Tarifas por fecha/rango (Fase C, ADR-0001).
 *
 * Cualquier item puede definir precios por temporada; la cotización agrupa los
 * días consecutivos con el mismo precio y usa el precio base cuando no hay
 * tarifa. Es la pieza de precio para hotel/alquiler; la disponibilidad sigue en
 * scheduling (recursos + citas por rango).
 */

const DIA_MS = 86_400_000;
const MAX_DIAS = 366;

export interface TarifaItem {
  id: string;
  empresa_id: string;
  item_id: string;
  desde: string;
  hasta: string;
  precio_cents: number;
  creado_en: string;
  actualizado_en: string;
}

export interface DesgloseTarifa {
  desde: string;
  hasta: string;
  dias: number;
  precio_cents: number;
  subtotal_cents: number;
}

export interface CotizacionItem {
  item_id: string;
  desde: string;
  hasta: string;
  dias: number;
  total_cents: number;
  desglose: DesgloseTarifa[];
}

export interface CrearTarifaInput {
  desde: string;
  hasta: string;
  precio_cents: number;
}

export type ActualizarTarifaInput = Partial<CrearTarifaInput>;

interface ItemPrecio {
  id: string;
  tipo: string;
  precio_cents: number;
}

function parseFecha(valor: unknown): Date | null {
  if (typeof valor !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(valor)) return null;
  const fecha = new Date(`${valor}T00:00:00Z`);
  if (Number.isNaN(fecha.getTime()) || fecha.toISOString().slice(0, 10) !== valor) return null;
  return fecha;
}

function formatear(fecha: Date): string {
  return fecha.toISOString().slice(0, 10);
}

function esConflictoDeRango(error: unknown): boolean {
  const codigo = (error as { code?: unknown })?.code;
  return codigo === '23P01' || codigo === '23505';
}

@Injectable()
export class TarifasService {
  constructor(private readonly pg: PgService) {}

  async listar(empresaId: string, itemId: string): Promise<TarifaItem[]> {
    await this.verificarItem(empresaId, itemId);
    return this.pg.query<TarifaItem>(
      `SELECT id, empresa_id, item_id, desde, hasta, precio_cents, creado_en, actualizado_en
         FROM catalog.item_tarifas
        WHERE empresa_id = $1 AND item_id = $2
        ORDER BY desde`,
      [empresaId, itemId],
    );
  }

  async crear(empresaId: string, itemId: string, input: CrearTarifaInput): Promise<TarifaItem> {
    await this.verificarItem(empresaId, itemId);
    const { desde, hasta, precio } = this.validarRango(input);
    try {
      const fila = await this.pg.queryOne<TarifaItem>(
        `INSERT INTO catalog.item_tarifas (empresa_id, item_id, desde, hasta, precio_cents)
         VALUES ($1, $2, $3::date, $4::date, $5)
         RETURNING id, empresa_id, item_id, desde, hasta, precio_cents, creado_en, actualizado_en`,
        [empresaId, itemId, desde, hasta, precio],
      );
      if (!fila) throw new DomainError('TARIFA_NO_CREADA', 'No se pudo guardar la tarifa.');
      return fila;
    } catch (error) {
      if (esConflictoDeRango(error)) {
        throw new ConflictError('La tarifa se solapa con otra del mismo item.');
      }
      throw error;
    }
  }

  async actualizar(
    empresaId: string,
    itemId: string,
    tarifaId: string,
    input: ActualizarTarifaInput,
  ): Promise<TarifaItem> {
    const actual = await this.buscar(empresaId, itemId, tarifaId);
    const { desde, hasta, precio } = this.validarRango({
      desde: input.desde ?? actual.desde,
      hasta: input.hasta ?? actual.hasta,
      precio_cents: input.precio_cents ?? actual.precio_cents,
    });
    try {
      const fila = await this.pg.queryOne<TarifaItem>(
        `UPDATE catalog.item_tarifas
            SET desde = $1::date, hasta = $2::date, precio_cents = $3, actualizado_en = NOW()
          WHERE id = $4 AND empresa_id = $5 AND item_id = $6
          RETURNING id, empresa_id, item_id, desde, hasta, precio_cents, creado_en, actualizado_en`,
        [desde, hasta, precio, tarifaId, empresaId, itemId],
      );
      if (!fila) throw new NotFoundError('Tarifa', tarifaId);
      return fila;
    } catch (error) {
      if (esConflictoDeRango(error)) {
        throw new ConflictError('La tarifa se solapa con otra del mismo item.');
      }
      throw error;
    }
  }

  async eliminar(empresaId: string, itemId: string, tarifaId: string): Promise<{ ok: true }> {
    await this.buscar(empresaId, itemId, tarifaId);
    const fila = await this.pg.queryOne<{ id: string }>(
      `DELETE FROM catalog.item_tarifas
        WHERE id = $1 AND empresa_id = $2 AND item_id = $3
        RETURNING id`,
      [tarifaId, empresaId, itemId],
    );
    if (!fila) throw new NotFoundError('Tarifa', tarifaId);
    return { ok: true };
  }

  async cotizar(
    empresaId: string,
    itemId: string,
    desdeValor: unknown,
    hastaValor: unknown,
  ): Promise<CotizacionItem> {
    const item = await this.verificarItem(empresaId, itemId);
    const desde = parseFecha(desdeValor);
    const hasta = parseFecha(hastaValor);
    if (!desde || !hasta) {
      throw new DomainError('RANGO_INVALIDO', 'Usa fechas YYYY-MM-DD.');
    }
    const dias = Math.round((hasta.getTime() - desde.getTime()) / DIA_MS) + 1;
    if (dias < 1) throw new DomainError('RANGO_INVALIDO', 'La fecha final no puede ser anterior.');
    if (dias > MAX_DIAS) {
      throw new DomainError('RANGO_INVALIDO', `El rango admite hasta ${MAX_DIAS} dias.`);
    }

    const tarifas = await this.pg.query<TarifaItem>(
      `SELECT id, empresa_id, item_id, desde, hasta, precio_cents, creado_en, actualizado_en
         FROM catalog.item_tarifas
        WHERE empresa_id = $1 AND item_id = $2
          AND daterange(desde, hasta, '[]') && daterange($3::date, $4::date, '[]')
        ORDER BY desde`,
      [empresaId, itemId, formatear(desde), formatear(hasta)],
    );

    const desglose: DesgloseTarifa[] = [];
    let total = 0;
    for (let i = 0; i < dias; i += 1) {
      const fecha = new Date(desde.getTime() + i * DIA_MS);
      const clave = formatear(fecha);
      const tarifa = tarifas.find((t) => t.desde <= clave && clave <= t.hasta);
      const precio = tarifa ? tarifa.precio_cents : item.precio_cents;
      const ultimo = desglose.at(-1);
      if (ultimo && ultimo.precio_cents === precio) {
        ultimo.hasta = clave;
        ultimo.dias += 1;
        ultimo.subtotal_cents += precio;
      } else {
        desglose.push({ desde: clave, hasta: clave, dias: 1, precio_cents: precio, subtotal_cents: precio });
      }
      total += precio;
    }

    return {
      item_id: itemId,
      desde: formatear(desde),
      hasta: formatear(hasta),
      dias,
      total_cents: total,
      desglose,
    };
  }

  private validarRango(input: CrearTarifaInput): { desde: string; hasta: string; precio: number } {
    const desde = parseFecha(input.desde);
    const hasta = parseFecha(input.hasta);
    if (!desde || !hasta) {
      throw new DomainError('RANGO_INVALIDO', 'Usa fechas YYYY-MM-DD.');
    }
    if (hasta.getTime() < desde.getTime()) {
      throw new DomainError('RANGO_INVALIDO', 'La fecha final no puede ser anterior.');
    }
    const precio = Number(input.precio_cents);
    if (!Number.isInteger(precio) || precio < 0) {
      throw new DomainError('PRECIO_INVALIDO', 'El precio debe ser un entero no negativo.');
    }
    return { desde: formatear(desde), hasta: formatear(hasta), precio };
  }

  private async verificarItem(empresaId: string, itemId: string): Promise<ItemPrecio> {
    const item = await this.pg.queryOne<ItemPrecio>(
      `SELECT id, tipo, precio_cents FROM catalog.items
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL`,
      [itemId, empresaId],
    );
    if (!item) throw new NotFoundError('Item', itemId);
    return item;
  }

  private async buscar(empresaId: string, itemId: string, tarifaId: string): Promise<TarifaItem> {
    const fila = await this.pg.queryOne<TarifaItem>(
      `SELECT id, empresa_id, item_id, desde, hasta, precio_cents, creado_en, actualizado_en
         FROM catalog.item_tarifas
        WHERE id = $1 AND empresa_id = $2 AND item_id = $3`,
      [tarifaId, empresaId, itemId],
    );
    if (!fila) throw new NotFoundError('Tarifa', tarifaId);
    return fila;
  }
}
