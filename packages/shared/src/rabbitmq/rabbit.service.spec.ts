import { describe, expect, it, vi } from 'vitest';
import { EVENTOS, EventoBus } from '../events/contracts';
import { EXCHANGE_EVENTS } from './rabbit.constants';
import { RabbitService } from './rabbit.service';

type CanalPublicacion = {
  publish: ReturnType<typeof vi.fn>;
  waitForConfirms: ReturnType<typeof vi.fn>;
};

function servicioConCanal(): { service: RabbitService; channel: CanalPublicacion } {
  const channel: CanalPublicacion = {
    publish: vi.fn().mockReturnValue(true),
    waitForConfirms: vi.fn().mockResolvedValue(undefined),
  };
  const service = new RabbitService('amqp://test');
  Reflect.set(service, 'canal', channel);
  return { service, channel };
}

function evento(request_id?: string): EventoBus {
  return {
    event_id: 'event-1',
    tipo: EVENTOS.ORDER_CREATED,
    ocurrido_en: '2026-10-14T12:00:00.000Z',
    ...(request_id ? { request_id } : {}),
    data: { order_id: 'order-1' },
  };
}

describe('RabbitService.publicar (R-OB-2)', () => {
  it('propaga x-request-id como header además de conservarlo en el payload', async () => {
    const { service, channel } = servicioConCanal();
    const message = evento('request-123');

    await expect(service.publicar(message)).resolves.toBe(true);

    expect(channel.publish).toHaveBeenCalledWith(
      EXCHANGE_EVENTS,
      EVENTOS.ORDER_CREATED,
      Buffer.from(JSON.stringify(message)),
      {
        persistent: true,
        contentType: 'application/json',
        headers: { 'x-request-id': 'request-123' },
      },
    );
    expect(channel.waitForConfirms).toHaveBeenCalledOnce();
  });

  it('no inventa un header cuando el evento no tiene contexto HTTP', async () => {
    const { service, channel } = servicioConCanal();

    await service.publicar(evento());

    expect(channel.publish).toHaveBeenCalledWith(
      EXCHANGE_EVENTS,
      EVENTOS.ORDER_CREATED,
      expect.any(Buffer),
      expect.objectContaining({ headers: undefined }),
    );
  });
});
