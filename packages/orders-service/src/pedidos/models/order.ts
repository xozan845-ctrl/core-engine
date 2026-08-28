import { ItemOrden, Money } from '@core/shared';

/**
 * Order: estructura de datos de la orden (modelo de dominio). El estado NO se
 * guarda como registro mutable: se reconstruye desde la secuencia de eventos.
 */
export interface OrderEstadoDerrivado {
  order_id: string;
  cliente_id: string;
  items: ItemOrden[];
  total: Money;
  estado: string;
  motivo?: string;
  creado_en: string;
  actualizado_en: string;
}

interface DatosDeEvento {
  order_id?: string;
  cliente_id?: string;
  items?: ItemOrden[];
  total_cents?: number;
  estado?: string;
  previo_estado?: string;
  motivo?: string;
  creado_en?: string;
  ocurrido_en?: string;
}

interface EventoReplay {
  tipo: string;
  payload: Record<string, unknown>;
  creado_en: string;
}

/** Reduce la historia de eventos a un estado (replay, patron Event Sourcing). */
export function reconstruirDesdeEventos(
  eventos: { tipo: string; payload: Record<string, unknown>; creado_en: string }[],
): OrderEstadoDerrivado | null {
  if (eventos.length === 0) return null;
  const primero = eventos[0];
  if (primero.tipo !== 'OrderCreatedEvent') return null;
  const inicial = primero.payload as DatosDeEvento;

  const orden: OrderEstadoDerrivado = {
    order_id: inicial.order_id ?? '',
    cliente_id: inicial.cliente_id ?? '',
    items: inicial.items ?? [],
    total: Money.desdeCentavos(inicial.total_cents ?? 0),
    estado: inicial.estado ?? 'creada',
    creado_en: inicial.creado_en ?? primero.creado_en,
    actualizado_en: primero.creado_en,
  };

  for (const evento of eventos.slice(1) as EventoReplay[]) {
    if (evento.tipo === 'OrderStatusUpdatedEvent') {
      const d = evento.payload as DatosDeEvento;
      if (d.estado) {
        orden.estado = d.estado;
        orden.motivo = d.motivo ?? undefined;
        orden.actualizado_en = evento.creado_en;
      }
    }
  }
  return orden;
}