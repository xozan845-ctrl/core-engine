import { Inject, Injectable } from '@nestjs/common';
import {
  EMAIL_PROVIDER,
  esUuid,
  Logger,
} from '@core/shared';
import type { EmailProvider, EventoBus } from '@core/shared';
import { ClientesService } from './clientes.service';
import { construirMensajes } from './plantillas';

/**
 * Orquesta una notificación: valida el pedido, resuelve comprador/vendedor por
 * las APIs internas, construye las plantillas y envía por el EmailProvider.
 * No marca idempotencia: eso lo hace el consumidor tras completar el envío.
 */
@Injectable()
export class NotificacionesService {
  private readonly logger = Logger.create('notifications.pedidos');

  constructor(
    private readonly clientes: ClientesService,
    @Inject(EMAIL_PROVIDER) private readonly email: EmailProvider,
  ) {}

  async procesar(evento: EventoBus): Promise<void> {
    const orderId = (evento.data as { order_id?: unknown } | null)?.order_id;
    if (typeof orderId !== 'string' || !esUuid(orderId)) {
      this.logger.warn({ msg: 'evento_sin_orden', tipo: evento.tipo });
      return;
    }

    const orden = await this.clientes.obtenerOrden(orderId);
    if (!orden) {
      // Sin proyección no hay a quién notificar; no se bloquea la cola.
      this.logger.warn({ msg: 'orden_no_disponible', order_id: orderId, tipo: evento.tipo });
      return;
    }

    const [comprador, vendedor] = await Promise.all([
      this.clientes.obtenerUsuario(orden.comprador_id),
      this.clientes.obtenerUsuario(orden.vendedor_id),
    ]);
    const mensajes = construirMensajes(evento, { orden, comprador, vendedor });
    for (const { mensaje } of mensajes) {
      await this.email.send(mensaje);
    }
    this.logger.info({
      msg: 'notificacion_enviada',
      tipo: evento.tipo,
      order_id: orderId,
      destinatarios: mensajes.map((m) => m.destinatario),
    });
  }
}
