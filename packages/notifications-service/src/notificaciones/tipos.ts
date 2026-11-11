import type { EmailMessage } from '@core/shared';

/** Orden mínima que necesita una notificación (vista interna de orders). */
export interface OrdenExterna {
  id: string;
  comprador_id: string;
  vendedor_id: string;
  estado: string;
  total_cents: number;
  moneda: string;
  items?: { producto_nombre?: string; cantidad: number }[];
}

/** Usuario mínimo que necesita una notificación (API interna de identity). */
export interface UsuarioExterno {
  id: string;
  nombre: string;
  correo: string;
}

export type Destinatario = 'comprador' | 'vendedor';

export interface MensajeNotificacion {
  destinatario: Destinatario;
  mensaje: EmailMessage;
}

export interface ContextoPedido {
  orden: OrdenExterna;
  comprador?: UsuarioExterno | null;
  vendedor?: UsuarioExterno | null;
}
