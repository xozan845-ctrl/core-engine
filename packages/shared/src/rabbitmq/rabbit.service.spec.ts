import { afterEach, describe, expect, it, vi } from 'vitest';
import * as amqp from 'amqplib';
import type { ChannelModel, ConsumeMessage } from 'amqplib';
import { EVENTOS, EventoBus } from '../events/contracts';
import {
  EXCHANGE_DLQ,
  EXCHANGE_EVENTS,
  REINTENTOS_MAXIMOS,
} from './rabbit.constants';
import { RabbitService } from './rabbit.service';

vi.mock('amqplib', () => ({ connect: vi.fn() }));

const connect = vi.mocked(amqp.connect);

type Mock = ReturnType<typeof vi.fn>;

type CanalFalso = {
  publish: Mock;
  waitForConfirms: Mock;
  assertExchange: Mock;
  assertQueue: Mock;
  bindQueue: Mock;
  prefetch: Mock;
  consume: Mock;
  get: Mock;
  ack: Mock;
  nack: Mock;
  close: Mock;
};

type ConexionFalsa = {
  on: Mock;
  createConfirmChannel: Mock;
  close: Mock;
};

function canalFalso(): CanalFalso {
  return {
    publish: vi.fn().mockReturnValue(true),
    waitForConfirms: vi.fn().mockResolvedValue(undefined),
    assertExchange: vi.fn().mockResolvedValue(undefined),
    assertQueue: vi.fn().mockResolvedValue(undefined),
    bindQueue: vi.fn().mockResolvedValue(undefined),
    prefetch: vi.fn().mockResolvedValue(undefined),
    consume: vi.fn().mockResolvedValue(undefined),
    get: vi.fn().mockResolvedValue(false),
    ack: vi.fn(),
    nack: vi.fn(),
    close: vi.fn().mockResolvedValue(undefined),
  };
}

function conexionFalsa(canal = canalFalso()): ConexionFalsa {
  return {
    on: vi.fn(),
    createConfirmChannel: vi.fn(async () => canal),
    close: vi.fn().mockResolvedValue(undefined),
  };
}

function servicioConCanal(canal = canalFalso()): {
  service: RabbitService;
  channel: CanalFalso;
} {
  const service = new RabbitService('amqp://test');
  Reflect.set(service, 'canal', canal);
  return { service, channel: canal as unknown as CanalFalso };
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

function mensaje(
  cuerpo: unknown,
  opciones: { headers?: Record<string, unknown>; routingKey?: string; texto?: boolean } = {},
): ConsumeMessage {
  return {
    content: Buffer.from(opciones.texto ? String(cuerpo) : JSON.stringify(cuerpo)),
    properties: { headers: opciones.headers ?? {} },
    fields: {
      routingKey: opciones.routingKey ?? EVENTOS.ORDER_CREATED,
      deliveryTag: 1,
      redelivered: false,
      exchange: EXCHANGE_EVENTS,
      consumerTag: 'consumer-test',
    },
  } as unknown as ConsumeMessage;
}

async function iniciarConsumo(
  service: RabbitService,
  channel: CanalFalso,
  nombreCola = 'catalog.pedidos',
): Promise<{ manejador: Mock; callback: (msg: ConsumeMessage | null) => Promise<void> }> {
  const manejador = vi.fn().mockResolvedValue(undefined);
  await service.consumir(nombreCola, manejador);
  const callback = channel.consume.mock.calls[0][1] as (
    msg: ConsumeMessage | null,
  ) => Promise<void>;
  return { manejador, callback };
}

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  connect.mockReset();
});

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

  it('devuelve false si el canal no acepta el mensaje pero confirma', async () => {
    const { service, channel } = servicioConCanal();
    channel.publish.mockReturnValue(false);

    await expect(service.publicar(evento())).resolves.toBe(false);
    expect(channel.waitForConfirms).toHaveBeenCalledOnce();
  });

  it('propaga el error si el broker no confirma la publicación', async () => {
    const { service, channel } = servicioConCanal();
    channel.waitForConfirms.mockRejectedValueOnce(new Error('sin confirmacion'));

    await expect(service.publicar(evento())).rejects.toThrow('sin confirmacion');
  });

  it('falla de forma controlada si el canal no está inicializado', async () => {
    const service = new RabbitService('amqp://test');

    await expect(service.publicar(evento())).rejects.toThrow(
      'Canal RabbitMQ no disponible',
    );
  });
});

describe('RabbitService · topología y consumo (R-AR-4)', () => {
  it('declara la cola, su DLQ y los bindings del exchange', async () => {
    const { service, channel } = servicioConCanal();

    await service.declararColas([
      {
        nombre: 'catalog.pedidos',
        cola: 'catalog.pedidos',
        routingKeys: ['order.created', 'devolucion.solicitada'],
      },
    ]);

    expect(channel.assertQueue).toHaveBeenNthCalledWith(1, 'catalog.pedidos', {
      durable: true,
      deadLetterExchange: EXCHANGE_DLQ,
      deadLetterRoutingKey: 'catalog.pedidos',
    });
    expect(channel.assertQueue).toHaveBeenNthCalledWith(2, 'catalog.pedidos.dlq', {
      durable: true,
    });
    expect(channel.bindQueue).toHaveBeenCalledWith(
      'catalog.pedidos',
      EXCHANGE_EVENTS,
      'order.created',
    );
    expect(channel.bindQueue).toHaveBeenCalledWith(
      'catalog.pedidos',
      EXCHANGE_EVENTS,
      'devolucion.solicitada',
    );
    expect(channel.bindQueue).toHaveBeenCalledWith(
      'catalog.pedidos.dlq',
      EXCHANGE_DLQ,
      'catalog.pedidos',
    );
  });

  it('confirma el mensaje cuando el manejador termina bien', async () => {
    const { service, channel } = servicioConCanal();
    const { manejador, callback } = await iniciarConsumo(service, channel);
    const original = evento();
    const msg = mensaje(original);

    await callback(msg);

    expect(manejador).toHaveBeenCalledWith(original);
    expect(channel.ack).toHaveBeenCalledWith(msg);
    expect(channel.publish).not.toHaveBeenCalled();
  });

  it('envía a DLQ con el intento incrementado si el manejador falla', async () => {
    const { service, channel } = servicioConCanal();
    const { manejador, callback } = await iniciarConsumo(service, channel);
    const msg = mensaje(evento(), {
      headers: { 'x-intentos': 1 },
      routingKey: 'order.created',
    });
    manejador.mockRejectedValueOnce(new Error('fallo de negocio'));

    await callback(msg);

    expect(channel.publish).toHaveBeenCalledWith(
      EXCHANGE_DLQ,
      'catalog.pedidos',
      msg.content,
      {
        persistent: true,
        headers: expect.objectContaining({
          'x-intentos': 2,
          'x-error': 'fallo de negocio',
          'x-cola': 'catalog.pedidos',
          'x-routing-key-original': 'order.created',
        }),
      },
    );
    expect(channel.waitForConfirms).toHaveBeenCalledOnce();
    expect(channel.ack).toHaveBeenCalledWith(msg);
    expect(channel.nack).not.toHaveBeenCalled();
  });

  it('marca como muerto el mensaje que supera los reintentos', async () => {
    const { service, channel } = servicioConCanal();
    const { manejador, callback } = await iniciarConsumo(service, channel);
    const msg = mensaje(evento(), { headers: { 'x-intentos': REINTENTOS_MAXIMOS } });
    manejador.mockRejectedValueOnce(new Error('sigue fallando'));

    await callback(msg);

    expect(channel.publish).toHaveBeenCalledWith(
      EXCHANGE_DLQ,
      'catalog.pedidos',
      msg.content,
      expect.objectContaining({
        headers: expect.objectContaining({
          'x-intentos': REINTENTOS_MAXIMOS + 1,
          'x-muerto': true,
        }),
      }),
    );
    expect(channel.ack).toHaveBeenCalledWith(msg);
  });

  it('manda a la DLQ un mensaje con JSON inválido sin invocar al manejador', async () => {
    const { service, channel } = servicioConCanal();
    const { manejador, callback } = await iniciarConsumo(service, channel);
    const msg = mensaje('{json roto', { texto: true });

    await callback(msg);

    expect(manejador).not.toHaveBeenCalled();
    expect(channel.publish).toHaveBeenCalledWith(
      EXCHANGE_DLQ,
      'catalog.pedidos',
      msg.content,
      expect.objectContaining({
        headers: expect.objectContaining({
          'x-intentos': REINTENTOS_MAXIMOS,
          'x-muerto': true,
        }),
      }),
    );
    expect(channel.ack).toHaveBeenCalledWith(msg);
  });

  it('devuelve el mensaje a la cola si la publicación en DLQ no se confirma', async () => {
    const { service, channel } = servicioConCanal();
    const { manejador, callback } = await iniciarConsumo(service, channel);
    const msg = mensaje(evento());
    manejador.mockRejectedValueOnce(new Error('fallo'));
    channel.waitForConfirms.mockRejectedValueOnce(new Error('broker caido'));

    await callback(msg);

    expect(channel.ack).not.toHaveBeenCalled();
    expect(channel.nack).toHaveBeenCalledWith(msg, false, true);
  });

  it('no reencola un mensaje corrupto si la DLQ tampoco está disponible', async () => {
    const { service, channel } = servicioConCanal();
    const { callback } = await iniciarConsumo(service, channel);
    const msg = mensaje('{json roto', { texto: true });
    channel.waitForConfirms.mockRejectedValueOnce(new Error('broker caido'));

    await callback(msg);

    expect(channel.ack).not.toHaveBeenCalled();
    expect(channel.nack).toHaveBeenCalledWith(msg, false, false);
  });
});

describe('RabbitService.reintentarDesdeDlq (AD-04, R-RB-3)', () => {
  it('reinyecta al exchange de eventos con la routing key original', async () => {
    vi.useFakeTimers();
    const { service, channel } = servicioConCanal();
    const msg = mensaje(evento(), {
      headers: { 'x-routing-key-original': 'order.actualizado' },
      routingKey: 'catalog.pedidos.dlq',
    });
    channel.get.mockResolvedValueOnce(msg).mockResolvedValueOnce(false);

    const promesa = service.reintentarDesdeDlq('catalog.pedidos');
    await vi.advanceTimersByTimeAsync(500);

    await expect(promesa).resolves.toBe(1);
    expect(channel.publish).toHaveBeenCalledWith(
      EXCHANGE_EVENTS,
      'order.actualizado',
      msg.content,
      expect.objectContaining({
        headers: expect.objectContaining({ 'x-reintentado': true }),
      }),
    );
    expect(channel.waitForConfirms).toHaveBeenCalledOnce();
    expect(channel.ack).toHaveBeenCalledWith(msg);
  });

  it('descarta los mensajes que ya agotaron los reintentos', async () => {
    const { service, channel } = servicioConCanal();
    const msg = mensaje(evento(), { headers: { 'x-intentos': REINTENTOS_MAXIMOS } });
    channel.get.mockResolvedValueOnce(msg).mockResolvedValueOnce(false);

    await expect(service.reintentarDesdeDlq('catalog.pedidos')).resolves.toBe(0);

    expect(channel.publish).not.toHaveBeenCalled();
    expect(channel.ack).toHaveBeenCalledWith(msg);
  });

  it('conserva el mensaje en la DLQ si falla la reinyección', async () => {
    vi.useFakeTimers();
    const { service, channel } = servicioConCanal();
    const msg = mensaje(evento(), { routingKey: 'catalog.pedidos.dlq' });
    channel.get.mockResolvedValueOnce(msg).mockResolvedValueOnce(false);
    channel.waitForConfirms.mockRejectedValueOnce(new Error('broker caido'));

    const promesa = service.reintentarDesdeDlq('catalog.pedidos');
    await vi.advanceTimersByTimeAsync(500);

    await expect(promesa).resolves.toBe(0);
    expect(channel.ack).not.toHaveBeenCalled();
    expect(channel.nack).toHaveBeenCalledWith(msg, false, true);
  });

  it('el poller recorre las colas declaradas y se detiene al destruir el módulo', async () => {
    vi.useFakeTimers();
    const { service, channel } = servicioConCanal();
    await service.declararColas([
      { nombre: 'catalog.pedidos', cola: 'catalog.pedidos', routingKeys: ['order.created'] },
    ]);
    const reintentar = vi.spyOn(service, 'reintentarDesdeDlq').mockResolvedValue(0);

    service.activarReintento(100);
    service.activarReintento(100);
    await vi.advanceTimersByTimeAsync(100);

    expect(reintentar).toHaveBeenCalledOnce();
    expect(reintentar).toHaveBeenCalledWith('catalog.pedidos');

    await service.onModuleDestroy();
    await vi.advanceTimersByTimeAsync(100);
    expect(reintentar).toHaveBeenCalledOnce();
    expect(channel.close).toHaveBeenCalledOnce();
  });
});

describe('RabbitService · conexión y reconexión', () => {
  it('conecta, declara los exchanges y cierra los recursos al destruir', async () => {
    const conexion = conexionFalsa();
    connect.mockResolvedValueOnce(conexion as unknown as ChannelModel);
    const service = new RabbitService('amqp://test');

    await service.onModuleInit();

    expect(connect).toHaveBeenCalledWith('amqp://test?heartbeat=30');
    expect(conexion.createConfirmChannel).toHaveBeenCalledOnce();
    expect(conexion.on).toHaveBeenCalledWith('close', expect.any(Function));
    expect(conexion.on).toHaveBeenCalledWith('error', expect.any(Function));

    await service.onModuleDestroy();
    expect(conexion.close).toHaveBeenCalledOnce();
  });

  it('reconecta con backoff tras cerrarse la conexión', async () => {
    vi.useFakeTimers();
    const primera = conexionFalsa();
    const segunda = conexionFalsa();
    connect
      .mockResolvedValueOnce(primera as unknown as ChannelModel)
      .mockResolvedValueOnce(segunda as unknown as ChannelModel);
    const service = new RabbitService('amqp://test');
    await service.onModuleInit();
    const alCerrar = primera.on.mock.calls.find(([evento]) => evento === 'close')?.[1] as (
      ...args: unknown[]
    ) => void;

    alCerrar();
    expect(connect).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1_000);
    expect(connect).toHaveBeenCalledTimes(2);
  });

  it('aplica backoff creciente si la reconexión falla', async () => {
    vi.useFakeTimers();
    const primera = conexionFalsa();
    const segunda = conexionFalsa();
    connect
      .mockResolvedValueOnce(primera as unknown as ChannelModel)
      .mockRejectedValueOnce(new Error('broker caido'))
      .mockResolvedValueOnce(segunda as unknown as ChannelModel);
    const service = new RabbitService('amqp://test');
    await service.onModuleInit();
    const alCerrar = primera.on.mock.calls.find(([evento]) => evento === 'close')?.[1] as (
      ...args: unknown[]
    ) => void;

    alCerrar();
    await vi.advanceTimersByTimeAsync(1_000);
    expect(connect).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(1_999);
    expect(connect).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(1);
    expect(connect).toHaveBeenCalledTimes(3);
  });

  it('no reconecta después de onModuleDestroy', async () => {
    vi.useFakeTimers();
    const primera = conexionFalsa();
    connect.mockResolvedValueOnce(primera as unknown as ChannelModel);
    const service = new RabbitService('amqp://test');
    await service.onModuleInit();
    const alCerrar = primera.on.mock.calls.find(([evento]) => evento === 'close')?.[1] as (
      ...args: unknown[]
    ) => void;

    alCerrar();
    await service.onModuleDestroy();
    await vi.advanceTimersByTimeAsync(30_000);

    expect(connect).toHaveBeenCalledTimes(1);
  });

  it('falla al arrancar sin URL y si el broker rechaza la conexión inicial', async () => {
    const sinUrl = new RabbitService('');
    await expect(sinUrl.onModuleInit()).rejects.toThrow('RABBITMQ_URL no esta definida');

    connect.mockRejectedValueOnce(new Error('broker caido'));
    const conUrl = new RabbitService('amqp://test');
    await expect(conUrl.onModuleInit()).rejects.toThrow('broker caido');
  });
});
