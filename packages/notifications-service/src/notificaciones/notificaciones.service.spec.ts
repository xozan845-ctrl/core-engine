import { EVENTOS } from '@core/shared';
import type { EmailProvider, EventoBus } from '@core/shared';
import type { ClientesService } from './clientes.service';
import { NotificacionesService } from './notificaciones.service';

const ORDEN = {
  id: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
  comprador_id: '11111111-1111-1111-1111-111111111111',
  vendedor_id: '22222222-2222-2222-2222-222222222222',
  estado: 'pagada',
  total_cents: 115000,
  moneda: 'NIO',
};

function evento(tipo: string, data: Record<string, unknown>): EventoBus {
  return {
    event_id: '33333333-3333-3333-3333-333333333333',
    tipo: tipo as EventoBus['tipo'],
    ocurrido_en: '2026-11-11T00:00:00.000Z',
    data,
  };
}

function crear() {
  const clientes = {
    obtenerOrden: vi.fn().mockResolvedValue(ORDEN),
    obtenerUsuario: vi.fn().mockImplementation(async (id: string) =>
      id === ORDEN.comprador_id
        ? { id, nombre: 'Ana', correo: 'ana@core.test' }
        : { id, nombre: 'Luis', correo: 'luis@core.test' },
    ),
  };
  const email = { send: vi.fn().mockResolvedValue(undefined) };
  const servicio = new NotificacionesService(
    clientes as unknown as ClientesService,
    email as unknown as EmailProvider,
  );
  return { servicio, clientes, email };
}

describe('NotificacionesService', () => {
  it('resuelve la orden y envía un correo por destinatario', async () => {
    const { servicio, clientes, email } = crear();

    await servicio.procesar(evento(EVENTOS.PAYMENT_PROCESADO, { order_id: ORDEN.id }));

    expect(clientes.obtenerOrden).toHaveBeenCalledWith(ORDEN.id);
    expect(clientes.obtenerUsuario).toHaveBeenCalledWith(ORDEN.comprador_id);
    expect(clientes.obtenerUsuario).toHaveBeenCalledWith(ORDEN.vendedor_id);
    expect(email.send).toHaveBeenCalledTimes(2);
    expect(email.send.mock.calls.map(([m]) => m.to)).toEqual(['ana@core.test', 'luis@core.test']);
  });

  it('ignora eventos sin order_id o con id inválido', async () => {
    const { servicio, clientes, email } = crear();

    await servicio.procesar(evento(EVENTOS.PAYMENT_PROCESADO, {}));
    await servicio.procesar(evento(EVENTOS.PAYMENT_PROCESADO, { order_id: '../../x' }));

    expect(clientes.obtenerOrden).not.toHaveBeenCalled();
    expect(email.send).not.toHaveBeenCalled();
  });

  it('no envía nada si la orden no está disponible', async () => {
    const { servicio, clientes, email } = crear();
    clientes.obtenerOrden.mockResolvedValue(null);

    await expect(
      servicio.procesar(evento(EVENTOS.PAYMENT_PROCESADO, { order_id: ORDEN.id })),
    ).resolves.toBeUndefined();
    expect(email.send).not.toHaveBeenCalled();
  });

  it('propaga el fallo de correo para que el consumidor reintente', async () => {
    const { servicio, email } = crear();
    email.send.mockRejectedValue(new Error('SMTP caído'));

    await expect(
      servicio.procesar(evento(EVENTOS.PAYMENT_PROCESADO, { order_id: ORDEN.id })),
    ).rejects.toThrow('SMTP caído');
  });

  it('no envía a usuarios sin correo', async () => {
    const { servicio, clientes, email } = crear();
    clientes.obtenerUsuario.mockResolvedValue(null);

    await expect(
      servicio.procesar(evento(EVENTOS.PAYMENT_PROCESADO, { order_id: ORDEN.id })),
    ).resolves.toBeUndefined();
    expect(email.send).not.toHaveBeenCalled();
  });
});
