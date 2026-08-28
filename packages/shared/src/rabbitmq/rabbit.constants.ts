/**
 * Topologia del bus (AD-02): RabbitMQ con colas por consumidor y DLQ (AD-04).
 */
export const EXCHANGE_EVENTS = 'bodegahub.events'; // topic
export const EXCHANGE_DLQ = 'bodegahub.dlq'; // topic

export interface ColaConfig {
  /** Nombre de la cola consumidora. */
  cola: string;
  /** Routing keys (nombres de evento) enlazados. */
  routingKeys: string[];
  /** Prefijo de la cola DLQ derivada. */
  dlq?: string;
}

/**
 * Colas por servicio consumidor (Tabla 22: consumidores por evento).
 */
export const COLAS: Record<string, ColaConfig> = {
  catalog_pedidos: {
    cola: 'catalog.pedidos',
    routingKeys: ['order.created', 'devolucion.solicitada'],
  },
  stores_stock: {
    cola: 'stores.stock',
    routingKeys: ['stock.updated'],
  },
  orders_resultados: {
    cola: 'orders.resultados',
    routingKeys: ['stock.reservado', 'stock.fallido', 'stock.reintegrado'],
  },
  logistics_pagos: {
    cola: 'logistics.pagos',
    routingKeys: ['payment.procesado', 'order.status.updated'],
  },
  commissions_pedidos: {
    cola: 'commissions.pedidos',
    routingKeys: ['order.completado', 'payment.procesado', 'devolucion.solicitada'],
  },
  commissions_liquidacion: {
    cola: 'commissions.liquidacion',
    routingKeys: ['comision.acreditada'],
  },
  finance_pedidos: {
    cola: 'finance.pedidos',
    routingKeys: ['order.created', 'payment.procesado', 'comision.acreditada', 'devolucion.solicitada'],
  },
  finance_contabilidad: {
    cola: 'finance.contabilidad',
    routingKeys: ['asiento.registrado'],
  },
  notifications_todos: {
    cola: 'notifications.todos',
    routingKeys: [
      'order.created',
      'order.status.updated',
      'payment.procesado',
      'order.completado',
      'shipment.started',
      'devolucion.solicitada',
      'stock.fallido',
    ],
  },
};

export const REINTENTOS_MAXIMOS = 3;
export const BACKOFF_BASE_MS = 500;