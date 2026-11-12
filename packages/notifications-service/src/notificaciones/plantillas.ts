import { EVENTOS } from '@core/shared';
import type { EventoBus } from '@core/shared';
import type {
  ContextoPedido,
  Destinatario,
  MensajeNotificacion,
  UsuarioExterno,
} from './tipos';

/**
 * Plantillas de correo del ciclo del pedido (ADR-0003).
 *
 * Son funciones puras: reciben el evento y el contexto resuelto por las APIs
 * internas, y devuelven los mensajes a enviar. No acceden a BD ni al bus.
 */

function dinero(cents: number, moneda: string): string {
  const simbolo = moneda === 'USD' ? 'US$' : 'C$';
  return `${simbolo} ${(cents / 100).toFixed(2)}`;
}

function escaparHtml(valor: string): string {
  return valor
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function pedidoCorto(id: string): string {
  return id.slice(0, 8);
}

function cuerpo(parrafos: string[]): { texto: string; html: string } {
  const texto = [...parrafos, '', 'Core Engine'].join('\n');
  const html = [
    ...parrafos.map((p) => `<p>${escaparHtml(p)}</p>`),
    '<p>Core Engine</p>',
  ].join('');
  return { texto, html };
}

function mensaje(
  destinatario: Destinatario,
  usuario: UsuarioExterno | null | undefined,
  asunto: string,
  contenido: { texto: string; html: string },
): MensajeNotificacion[] {
  if (!usuario?.correo) return [];
  return [
    {
      destinatario,
      mensaje: {
        to: usuario.correo,
        subject: asunto,
        text: contenido.texto,
        html: `<h1>${escaparHtml(asunto)}</h1>${contenido.html}`,
      },
    },
  ];
}

function totalOrden(contexto: ContextoPedido): string {
  return dinero(contexto.orden.total_cents, contexto.orden.moneda);
}

function estadoLegible(estado: string): string {
  const mapa: Record<string, string> = {
    creada: 'creado',
    pagada: 'pagado',
    en_preparacion: 'en preparación',
    enviada: 'enviado',
    en_camino: 'en camino',
    entregada: 'entregado',
    cancelada: 'cancelado',
    devuelta: 'devuelto',
    devolucion_solicitada: 'con devolución solicitada',
  };
  return mapa[estado] ?? estado;
}

function asuntoEstado(estado: string, corto: string): string {
  if (['enviada', 'en_camino', 'en_transito'].includes(estado)) {
    return `Tu pedido está en camino · #${corto}`;
  }
  if (estado === 'entregada') return `Tu pedido fue entregado · #${corto}`;
  if (estado === 'cancelada') return `Tu pedido fue cancelado · #${corto}`;
  if (estado === 'devuelta' || estado === 'devolucion_solicitada') {
    return `Devolución registrada · #${corto}`;
  }
  return `Actualización de tu pedido · #${corto}`;
}

/**
 * Construye los mensajes del evento. Los eventos sin plantilla (p. ej.
 * `order.created`) no generan correo.
 */
export function construirMensajes(
  evento: EventoBus,
  contexto: ContextoPedido,
): MensajeNotificacion[] {
  const { orden, comprador, vendedor } = contexto;
  const corto = pedidoCorto(orden.id);

  switch (evento.tipo) {
    case EVENTOS.PAYMENT_PROCESADO: {
      const compra = cuerpo([
        `Recibimos el pago de tu pedido por ${totalOrden(contexto)}.`,
        'El vendedor ya fue notificado para prepararlo.',
      ]);
      const venta = cuerpo([
        `Tienes un pedido pagado por ${totalOrden(contexto)}.`,
        'Prepara el envío desde tu panel de vendedor.',
      ]);
      return [
        ...mensaje('comprador', comprador, `Pago confirmado · #${corto}`, compra),
        ...mensaje('vendedor', vendedor, `Nuevo pedido pagado · #${corto}`, venta),
      ];
    }

    case EVENTOS.SHIPMENT_STARTED: {
      const guia = (evento.data as { guia?: unknown }).guia;
      const detalle =
        typeof guia === 'string' && guia
          ? `Guía de despacho: ${guia}.`
          : 'Estamos preparando el envío.';
      const compra = cuerpo([detalle, `Total del pedido: ${totalOrden(contexto)}.`]);
      return mensaje('comprador', comprador, `Tu pedido está en preparación · #${corto}`, compra);
    }

    case EVENTOS.ORDER_STATUS_UPDATED: {
      const data = evento.data as { estado?: unknown; motivo?: unknown };
      const estado = typeof data.estado === 'string' ? data.estado : orden.estado;
      const motivo =
        typeof data.motivo === 'string' && data.motivo ? ` Motivo: ${data.motivo}.` : '';
      const asunto = asuntoEstado(estado, corto);
      const compra = cuerpo([
        `Tu pedido #${corto} pasó a estado "${estadoLegible(estado)}".${motivo}`,
        `Total: ${totalOrden(contexto)}.`,
      ]);
      const mensajes = mensaje('comprador', comprador, asunto, compra);
      if (estado === 'cancelada' || estado === 'devuelta') {
        const venta = cuerpo([
          `El pedido #${corto} pasó a estado "${estadoLegible(estado)}".${motivo}`,
        ]);
        mensajes.push(
          ...mensaje('vendedor', vendedor, `Pedido ${estadoLegible(estado)} · #${corto}`, venta),
        );
      }
      return mensajes;
    }

    case EVENTOS.ORDER_COMPLETADO: {
      const compra = cuerpo([
        'Tu pedido fue entregado. ¡Gracias por comprar!',
        `Total: ${totalOrden(contexto)}.`,
      ]);
      return mensaje('comprador', comprador, `Pedido entregado · #${corto}`, compra);
    }

    case EVENTOS.DEVOLUCION_SOLICITADA: {
      const motivo = (evento.data as { motivo?: unknown }).motivo;
      const detalle = typeof motivo === 'string' && motivo ? ` Motivo: ${motivo}.` : '';
      const compra = cuerpo([`Registramos la devolución de tu pedido.${detalle}`]);
      return mensaje('comprador', comprador, `Devolución solicitada · #${corto}`, compra);
    }

    case EVENTOS.STOCK_FALLIDO: {
      const compra = cuerpo([
        'No pudimos reservar todos los productos de tu pedido.',
        'El pedido será cancelado y el pago revertido si corresponde.',
      ]);
      return mensaje('comprador', comprador, `No pudimos reservar el stock · #${corto}`, compra);
    }

    default:
      return [];
  }
}
