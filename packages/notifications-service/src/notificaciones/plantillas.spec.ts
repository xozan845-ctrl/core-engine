import { EVENTOS } from '@core/shared';
import type { EventoBus } from '@core/shared';
import { construirMensajes } from './plantillas';
import type { ContextoPedido } from './tipos';

const ORDEN = {
  id: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
  comprador_id: '11111111-1111-1111-1111-111111111111',
  vendedor_id: '22222222-2222-2222-2222-222222222222',
  estado: 'pagada',
  total_cents: 115000,
  moneda: 'NIO',
};

const COMPRADOR = { id: ORDEN.comprador_id, nombre: 'Ana', correo: 'ana@core.test' };
const VENDEDOR = { id: ORDEN.vendedor_id, nombre: 'Luis', correo: 'luis@core.test' };

function contexto(extra: Partial<ContextoPedido> = {}): ContextoPedido {
  return { orden: ORDEN, comprador: COMPRADOR, vendedor: VENDEDOR, ...extra };
}

function evento(tipo: string, data: Record<string, unknown>): EventoBus {
  return {
    event_id: '33333333-3333-3333-3333-333333333333',
    tipo: tipo as EventoBus['tipo'],
    ocurrido_en: '2026-11-11T00:00:00.000Z',
    data,
  };
}

describe('construirMensajes (ADR-0003)', () => {
  it('el pago confirmado avisa al comprador y al vendedor', () => {
    const mensajes = construirMensajes(
      evento(EVENTOS.PAYMENT_PROCESADO, { order_id: ORDEN.id, monto_cents: 115000 }),
      contexto(),
    );

    expect(mensajes.map((m) => [m.destinatario, m.mensaje.to])).toEqual([
      ['comprador', 'ana@core.test'],
      ['vendedor', 'luis@core.test'],
    ]);
    expect(mensajes[0].mensaje.subject).toBe('Pago confirmado · #aaaaaaaa');
    expect(mensajes[0].mensaje.text).toContain('C$ 1150.00');
  });

  it('el envío iniciado incluye la guía y solo va al comprador', () => {
    const mensajes = construirMensajes(
      evento(EVENTOS.SHIPMENT_STARTED, { order_id: ORDEN.id, guia: 'GUIA-123' }),
      contexto(),
    );

    expect(mensajes).toHaveLength(1);
    expect(mensajes[0].mensaje.text).toContain('GUIA-123');
    expect(mensajes[0].mensaje.subject).toBe('Tu pedido está en preparación · #aaaaaaaa');
  });

  it.each([
    ['enviada', 'Tu pedido está en camino · #aaaaaaaa', 1],
    ['entregada', 'Tu pedido fue entregado · #aaaaaaaa', 1],
    ['cancelada', 'Tu pedido fue cancelado · #aaaaaaaa', 2],
    ['devuelta', 'Devolución registrada · #aaaaaaaa', 2],
    ['pagada', 'Actualización de tu pedido · #aaaaaaaa', 1],
  ])('mapea el estado %s a su asunto', (estado, asunto, cantidad) => {
    const mensajes = construirMensajes(
      evento(EVENTOS.ORDER_STATUS_UPDATED, { order_id: ORDEN.id, estado }),
      contexto(),
    );

    expect(mensajes).toHaveLength(cantidad);
    expect(mensajes[0].mensaje.subject).toBe(asunto);
  });

  it('escapa el motivo en el HTML y lo incluye en el texto', () => {
    const mensajes = construirMensajes(
      evento(EVENTOS.ORDER_STATUS_UPDATED, {
        order_id: ORDEN.id,
        estado: 'cancelada',
        motivo: '<script>alert(1)</script>',
      }),
      contexto(),
    );

    expect(mensajes[0].mensaje.text).toContain('<script>alert(1)</script>');
    expect(mensajes[0].mensaje.html).toContain('&lt;script&gt;alert(1)&lt;/script&gt;');
    expect(mensajes[0].mensaje.html).not.toContain('<script>');
  });

  it('notifica la entrega, la devolución y el stock fallido', () => {
    const entrega = construirMensajes(
      evento(EVENTOS.ORDER_COMPLETADO, { order_id: ORDEN.id }),
      contexto(),
    );
    const devolucion = construirMensajes(
      evento(EVENTOS.DEVOLUCION_SOLICITADA, { order_id: ORDEN.id, motivo: 'dañado' }),
      contexto(),
    );
    const stock = construirMensajes(
      evento(EVENTOS.STOCK_FALLIDO, { order_id: ORDEN.id }),
      contexto(),
    );

    expect(entrega[0].mensaje.subject).toBe('Pedido entregado · #aaaaaaaa');
    expect(devolucion[0].mensaje.text).toContain('dañado');
    expect(stock[0].mensaje.subject).toBe('No pudimos reservar el stock · #aaaaaaaa');
  });

  it('no genera correo sin destinatario ni para eventos sin plantilla', () => {
    expect(
      construirMensajes(
        evento(EVENTOS.PAYMENT_PROCESADO, { order_id: ORDEN.id }),
        contexto({ comprador: null, vendedor: null }),
      ),
    ).toEqual([]);
    expect(
      construirMensajes(evento(EVENTOS.ORDER_CREATED, { order_id: ORDEN.id }), contexto()),
    ).toEqual([]);
  });

  it('usa US$ cuando la orden está en dólares', () => {
    const mensajes = construirMensajes(
      evento(EVENTOS.PAYMENT_PROCESADO, { order_id: ORDEN.id }),
      contexto({ orden: { ...ORDEN, moneda: 'USD' } }),
    );
    expect(mensajes[0].mensaje.text).toContain('US$ 1150.00');
  });
});
