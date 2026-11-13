import { Injectable } from '@nestjs/common';
import {
  ConflictError,
  DomainError,
  ForbiddenError,
  NotFoundError,
  PgService,
  Rol,
  ROLES,
} from '@core/shared';

/**
 * Entrega digital (Fase P2.1).
 *
 * El acceso nace al crear la orden con fulfillment `digital`, pero solo se
 * puede consumir cuando la orden esta pagada. El token es una capacidad de
 * descarga con limites opcionales de cantidad y expiracion.
 */

export interface EntregaDigital {
  id: string;
  order_id: string;
  item_id: string;
  sku: string;
  token: string;
  estado: 'activo' | 'revocado';
  descargas: number;
  max_descargas: number | null;
  expira_en: string | null;
  primera_descarga_en: string | null;
  ultima_descarga_en: string | null;
  creado_en: string;
}

export interface DescargaDigital extends EntregaDigital {
  comprador_id: string;
  recurso_url: string;
}

interface Actor {
  user_id: string;
  rol: Rol;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const ESTADOS_PAGADOS = ['pagada', 'enviada', 'entregada'];

@Injectable()
export class EntregasDigitalesService {
  constructor(private readonly pg: PgService) {}

  async listarPorOrden(orderId: string, actor: Actor): Promise<EntregaDigital[]> {
    this.validarUuid(orderId, 'orden');
    const orden = await this.pg.queryOne<{ comprador_id: string }>(
      `SELECT comprador_id FROM orders.orden_vista WHERE id = $1`,
      [orderId],
    );
    if (!orden) throw new NotFoundError('Orden', orderId);
    this.autorizar(actor, orden.comprador_id);
    return this.pg.query<EntregaDigital>(
      `SELECT id, order_id, item_id, sku, token, estado, descargas, max_descargas,
              expira_en, primera_descarga_en, ultima_descarga_en, creado_en
         FROM orders.entregas_digitales
        WHERE order_id = $1
        ORDER BY creado_en, sku`,
      [orderId],
    );
  }

  async descargar(token: string, actor: Actor): Promise<DescargaDigital> {
    this.validarUuid(token, 'token');
    const entrega = await this.pg.queryOne<DescargaDigital & { order_estado: string }>(
      `SELECT e.id, e.order_id, e.item_id, e.sku, e.token, e.estado, e.descargas,
              e.max_descargas, e.expira_en, e.primera_descarga_en,
              e.ultima_descarga_en, e.creado_en, e.recurso_url, e.comprador_id,
              o.estado AS order_estado
         FROM orders.entregas_digitales e
         JOIN orders.orden_vista o ON o.id = e.order_id
        WHERE e.token = $1`,
      [token],
    );
    if (!entrega) throw new NotFoundError('Entrega digital', token);
    this.autorizar(actor, entrega.comprador_id);
    if (entrega.estado !== 'activo') {
      throw new ConflictError('El acceso digital esta revocado.', undefined, 'ENTREGA_REVOCADA');
    }
    if (!ESTADOS_PAGADOS.includes(entrega.order_estado)) {
      throw new ConflictError(
        'La orden aun no esta pagada; la entrega digital no esta disponible.',
        undefined,
        'ORDEN_NO_PAGADA',
      );
    }
    if (entrega.expira_en && new Date(entrega.expira_en).getTime() <= Date.now()) {
      throw new ConflictError('El acceso digital expiro.', undefined, 'ENTREGA_EXPIRADA');
    }
    if (
      entrega.max_descargas !== null
      && entrega.descargas >= entrega.max_descargas
    ) {
      throw new ConflictError(
        'El acceso digital alcanzo el maximo de descargas.',
        undefined,
        'ENTREGA_SIN_DESCARGAS',
      );
    }

    const actualizada = await this.pg.queryOne<DescargaDigital>(
      `UPDATE orders.entregas_digitales
          SET descargas = descargas + 1,
              primera_descarga_en = COALESCE(primera_descarga_en, NOW()),
              ultima_descarga_en = NOW()
        WHERE token = $1 AND estado = 'activo'
          AND (max_descargas IS NULL OR descargas < max_descargas)
          AND (expira_en IS NULL OR expira_en > NOW())
        RETURNING id, order_id, item_id, sku, token, estado, descargas,
                  max_descargas, expira_en, primera_descarga_en,
                  ultima_descarga_en, creado_en, recurso_url`,
      [token],
    );
    if (!actualizada) {
      throw new ConflictError(
        'El acceso digital ya no esta disponible.',
        undefined,
        'ENTREGA_SIN_DESCARGAS',
      );
    }
    return actualizada;
  }

  private autorizar(actor: Actor, compradorId: string): void {
    if (actor.rol === ROLES.SUPER_ADMIN || actor.rol === ROLES.ADMIN) return;
    if (actor.rol !== ROLES.COMPRADOR || actor.user_id !== compradorId) {
      throw new ForbiddenError('Solo el comprador de la orden puede acceder a la entrega digital.');
    }
  }

  private validarUuid(valor: unknown, campo: string): string {
    const id = String(valor ?? '');
    if (!UUID.test(id)) throw new DomainError('ID_INVALIDO', `El ${campo} debe ser un UUID.`);
    return id;
  }
}
