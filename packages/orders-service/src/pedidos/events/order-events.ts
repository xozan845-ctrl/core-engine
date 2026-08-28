import { ItemOrden } from '@core/shared';

/**
 * OrderCreatedEvent: estado de la orden justo despues de crearse.
 * Es la fuente de verdad (Event Sourcing) del nacimiento de la orden.
 */
export interface OrderCreatedEvent {
  order_id: string;
  cliente_id: string;
  items: ItemOrden[];
  total_cents: number;
  estado: 'creada';
  creado_en: string;
}

/**
 * OrderStatusUpdatedEvent: transiciones de estado posteriores (Tabla 13).
 */
export interface OrderStatusUpdatedEvent {
  order_id: string;
  estado: string;
  previo_estado: string;
  motivo?: string;
  ocurrido_en: string;
}