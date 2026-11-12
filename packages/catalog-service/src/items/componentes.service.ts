import { Injectable } from '@nestjs/common';
import { ConflictError, DomainError, NotFoundError, PgService } from '@core/shared';

/**
 * Composición de items (Fase D, ADR-0001).
 *
 * Un item compuesto (plato, kit, combo) consume sus componentes al venderlo.
 * El grafo es acíclico y cada componente debe ser un producto de la empresa.
 */

export interface ComponenteItem {
  id: string;
  empresa_id: string;
  item_id: string;
  componente_item_id: string;
  codigo: string;
  nombre: string;
  stock: number;
  cantidad: number;
  creado_en: string;
  actualizado_en: string;
}

export interface CrearComponenteInput {
  componente_item_id: string;
  cantidad: number;
}

interface ItemBasico {
  id: string;
  tipo: string;
  codigo: string;
  nombre: string;
  stock: number;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

@Injectable()
export class ComponentesService {
  constructor(private readonly pg: PgService) {}

  async listar(empresaId: string, itemId: string): Promise<ComponenteItem[]> {
    await this.verificarItem(empresaId, itemId);
    return this.pg.query<ComponenteItem>(
      `SELECT c.id, c.empresa_id, c.item_id, c.componente_item_id,
              i.codigo, i.nombre, i.stock, c.cantidad, c.creado_en, c.actualizado_en
         FROM catalog.item_componentes c
         JOIN catalog.items i ON i.id = c.componente_item_id
        WHERE c.empresa_id = $1 AND c.item_id = $2
        ORDER BY i.nombre`,
      [empresaId, itemId],
    );
  }

  async crear(
    empresaId: string,
    itemId: string,
    input: CrearComponenteInput,
  ): Promise<ComponenteItem> {
    await this.verificarItem(empresaId, itemId);
    const componenteId = String(input.componente_item_id ?? '');
    if (!UUID.test(componenteId)) {
      throw new DomainError('COMPONENTE_INVALIDO', 'El componente debe ser un UUID.');
    }
    if (componenteId === itemId) {
      throw new DomainError('COMPONENTE_INVALIDO', 'Un item no puede componerse de si mismo.');
    }
    const cantidad = this.validarCantidad(input.cantidad);
    const componente = await this.verificarItem(empresaId, componenteId);
    // El padre puede ser producto, servicio o alquiler (una reparacion consume
    // repuestos); los componentes siempre deben ser productos inventariables.
    if (componente.tipo !== 'producto') {
      throw new DomainError(
        'COMPONENTE_INVALIDO',
        'Los componentes deben ser productos de la empresa.',
      );
    }
    if (await this.hayCiclo(empresaId, itemId, componenteId)) {
      throw new ConflictError('La composicion generaria un ciclo.');
    }
    try {
      const fila = await this.pg.queryOne<ComponenteItem>(
        `WITH insertado AS (
           INSERT INTO catalog.item_componentes (empresa_id, item_id, componente_item_id, cantidad)
           VALUES ($1, $2, $3, $4)
           RETURNING id, empresa_id, item_id, componente_item_id, cantidad, creado_en, actualizado_en
         )
         SELECT i.id, i.empresa_id, i.item_id, i.componente_item_id,
                it.codigo, it.nombre, it.stock, i.cantidad, i.creado_en, i.actualizado_en
           FROM insertado i JOIN catalog.items it ON it.id = i.componente_item_id`,
        [empresaId, itemId, componenteId, cantidad],
      );
      if (!fila) throw new DomainError('COMPONENTE_NO_CREADO', 'No se pudo guardar el componente.');
      return fila;
    } catch (error) {
      if ((error as { code?: string })?.code === '23505') {
        throw new ConflictError('Ese producto ya es componente del item.');
      }
      throw error;
    }
  }

  async actualizar(
    empresaId: string,
    itemId: string,
    componenteId: string,
    input: { cantidad: number },
  ): Promise<ComponenteItem> {
    await this.verificarItem(empresaId, itemId);
    const cantidad = this.validarCantidad(input.cantidad);
    const fila = await this.pg.queryOne<ComponenteItem>(
      `UPDATE catalog.item_componentes
          SET cantidad = $1, actualizado_en = NOW()
        WHERE id = $2 AND empresa_id = $3 AND item_id = $4
        RETURNING id`,
      [cantidad, componenteId, empresaId, itemId],
    );
    if (!fila) throw new NotFoundError('Componente', componenteId);
    return this.buscar(empresaId, itemId, componenteId);
  }

  async eliminar(empresaId: string, itemId: string, componenteId: string): Promise<{ ok: true }> {
    await this.verificarItem(empresaId, itemId);
    const fila = await this.pg.queryOne<{ id: string }>(
      `DELETE FROM catalog.item_componentes
        WHERE id = $1 AND empresa_id = $2 AND item_id = $3
        RETURNING id`,
      [componenteId, empresaId, itemId],
    );
    if (!fila) throw new NotFoundError('Componente', componenteId);
    return { ok: true };
  }

  private async buscar(
    empresaId: string,
    itemId: string,
    componenteId: string,
  ): Promise<ComponenteItem> {
    const fila = await this.pg.queryOne<ComponenteItem>(
      `SELECT c.id, c.empresa_id, c.item_id, c.componente_item_id,
              i.codigo, i.nombre, i.stock, c.cantidad, c.creado_en, c.actualizado_en
         FROM catalog.item_componentes c
         JOIN catalog.items i ON i.id = c.componente_item_id
        WHERE c.id = $1 AND c.empresa_id = $2 AND c.item_id = $3`,
      [componenteId, empresaId, itemId],
    );
    if (!fila) throw new NotFoundError('Componente', componenteId);
    return fila;
  }

  private async verificarItem(empresaId: string, itemId: string): Promise<ItemBasico> {
    const item = await this.pg.queryOne<ItemBasico>(
      `SELECT id, tipo, codigo, nombre, stock FROM catalog.items
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL`,
      [itemId, empresaId],
    );
    if (!item) throw new NotFoundError('Item', itemId);
    return item;
  }

  /** ¿El item padre es alcanzable desde el componente? (evita ciclos). */
  private async hayCiclo(
    empresaId: string,
    itemId: string,
    componenteId: string,
  ): Promise<boolean> {
    const fila = await this.pg.queryOne<{ uno: number }>(
      `WITH RECURSIVE alcanzables AS (
         SELECT componente_item_id
           FROM catalog.item_componentes
          WHERE item_id = $1 AND empresa_id = $3
         UNION
         SELECT c.componente_item_id
           FROM catalog.item_componentes c
           JOIN alcanzables a ON c.item_id = a.componente_item_id
          WHERE c.empresa_id = $3
       )
       SELECT 1 AS uno FROM alcanzables WHERE componente_item_id = $2 LIMIT 1`,
      [componenteId, itemId, empresaId],
    );
    return !!fila;
  }

  private validarCantidad(valor: unknown): number {
    const cantidad = Number(valor);
    if (!Number.isInteger(cantidad) || cantidad < 1) {
      throw new DomainError('COMPONENTE_CANTIDAD_INVALIDA', 'La cantidad debe ser un entero positivo.');
    }
    return cantidad;
  }
}
