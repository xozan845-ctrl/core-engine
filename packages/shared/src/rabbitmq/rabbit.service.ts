import { Injectable, OnModuleDestroy, OnModuleInit, Optional } from '@nestjs/common';
import * as amqp from 'amqplib';
import type { ChannelModel, ConfirmChannel } from 'amqplib';
import { Logger } from '../logging/logger';
import {
  ColaConfig,
  EXCHANGE_DLQ,
  EXCHANGE_EVENTS,
  REINTENTOS_MAXIMOS,
  BACKOFF_BASE_MS,
} from './rabbit.constants';
import { NombreEvento, EventoBus } from '../events/contracts';

export interface ColaConNombre extends ColaConfig {
  nombre: string;
}

/**
 * Cliente RabbitMQ: declara el exchange topic de eventos, la DLQ y las colas
 * que este servicio consume (con dead-letter hacia la DLQ, AD-04).
 */
@Injectable()
export class RabbitService implements OnModuleInit, OnModuleDestroy {
  private conexion?: ChannelModel;
  private canal?: ConfirmChannel;
  private readonly logger = Logger.create('rabbitmq');
  private colasDeclaradas = new Set<string>();
  private timerReintento?: NodeJS.Timeout;

  constructor(@Optional() private readonly url?: string) {
    this.url = this.url ?? process.env.RABBITMQ_URL ?? '';
  }

  async onModuleInit(): Promise<void> {
    await this.conectar();
  }

  async onModuleDestroy(): Promise<void> {
    if (this.timerReintento) clearInterval(this.timerReintento);
    try {
      if (this.canal) await this.canal.close();
      if (this.conexion) await this.conexion.close();
    } catch {
      /* cierre en frio */
    }
  }

  private async conectar(): Promise<void> {
    if (!this.url) {
      throw new Error('RABBITMQ_URL no esta definida.');
    }
    this.conexion = await amqp.connect(this.url, { heartbeat: 30 });
    this.conexion.on('close', () => {
      this.logger.warn({ msg: 'Conexion al broker cerrada; reintentando en 5s' });
      void this.conectar().catch((e) => this.logger.error({ msg: 'reconexion', err: e.message }));
    });
    this.canal = await this.conexion.createConfirmChannel();
    await this.canal.assertExchange(EXCHANGE_EVENTS, 'topic', { durable: true });
    await this.canal.assertExchange(EXCHANGE_DLQ, 'topic', { durable: true });
    this.logger.info({ msg: 'Broker conectado', exchange: EXCHANGE_EVENTS });
  }

  private canalListo(): ConfirmChannel {
    if (!this.canal) {
      throw new Error('Canal RabbitMQ no disponible.');
    }
    return this.canal;
  }

  /** Declara una cola de consumo con su DLQ (x-dead-letter-exchange). */
  async declararColas(colas: ColaConNombre[]): Promise<void> {
    const canal = this.canalListo();
    for (const config of colas) {
      const dlqNombre = `${config.nombre}.dlq`;
      await canal.assertQueue(config.nombre, {
        durable: true,
        deadLetterExchange: EXCHANGE_DLQ,
        deadLetterRoutingKey: config.nombre,
      });
      await canal.assertQueue(dlqNombre, { durable: true });
      for (const key of config.routingKeys) {
        await canal.bindQueue(config.nombre, EXCHANGE_EVENTS, key);
      }
      await canal.bindQueue(dlqNombre, EXCHANGE_DLQ, config.nombre);
      this.colasDeclaradas.add(config.nombre);
    }
  }

  /** Publica un evento con confirmacion (el outbox reaprecia si falla). */
  async publicar<T>(evento: EventoBus<T>): Promise<boolean> {
    const canal = this.canalListo();
    const ok = canal.publish(
      EXCHANGE_EVENTS,
      evento.tipo,
      Buffer.from(JSON.stringify(evento)),
      { persistent: true, contentType: 'application/json' },
    );
    await canal.waitForConfirms();
    return ok;
  }

  /** Suscripcion con backoff exponencial y DLQ (AD-04). */
  async consumir(nombreCola: string, manejador: (mensaje: EventoBus) => Promise<void>): Promise<void> {
    const canal = this.canalListo();
    await canal.prefetch(1);
    await canal.consume(nombreCola, async (msg) => {
      if (!msg) return;
      const cuerpo = JSON.parse(msg.content.toString()) as EventoBus;
      const intentos = (msg.properties.headers?.['x-intentos'] ?? 0) as number;
      try {
        await manejador(cuerpo);
        canal.ack(msg);
      } catch (err) {
        const siguiente = intentos + 1;
        const errMsg = err instanceof Error ? err.message : String(err);
        this.logger.error({
          msg: 'Mensaje fallido',
          cola: nombreCola,
          evento: cuerpo.tipo,
          intentos: siguiente,
          err: errMsg,
        });
        if (siguiente <= REINTENTOS_MAXIMOS) {
          // backoff exponencial: publicar en la cola DLQ y desde alli se reinyecta
          canal.publish(
            EXCHANGE_DLQ,
            nombreCola,
            msg.content,
            {
              persistent: true,
              headers: {
                ...msg.properties.headers,
                'x-intentos': siguiente,
                'x-error': errMsg,
                'x-cola': nombreCola,
                'x-routing-key-original': msg.fields.routingKey,
              },
            },
          );
        } else {
          canal.publish(EXCHANGE_DLQ, nombreCola, msg.content, {
            persistent: true,
            headers: {
              ...msg.properties.headers,
              'x-intentos': siguiente,
              'x-error': errMsg,
              'x-cola': nombreCola,
              'x-routing-key-original': msg.fields.routingKey,
              'x-muerto': true,
            },
          });
        }
        canal.ack(msg);
      }
    });
  }

  /**
   * Reprocesa la DLQ: reinyeccion manual/automatica con backoff (AD-04).
   * Devuelve la cantidad de mensajes reinyectados o descartados (muertos).
   */
  async reintentarDesdeDlq(nombreCola: string): Promise<number> {
    const canal = this.canalListo();
    const dlq = `${nombreCola}.dlq`;
    let reprocesados = 0;
    for (;;) {
      const msg = await canal.get(dlq, { noAck: false });
      if (!msg) break;
      const intentos = (msg.properties.headers?.['x-intentos'] ?? 0) as number;
      const espera = Math.min(30000, BACKOFF_BASE_MS * Math.pow(2, intentos));
      if (intentos >= REINTENTOS_MAXIMOS) {
        this.logger.error({
          msg: 'Mensaje descartado (muerto) tras maximos intentos',
          cola: nombreCola,
          routing_key: msg.fields.routingKey,
        });
        canal.ack(msg);
        continue;
      }
      const routingKey =
        (msg.properties.headers?.['x-routing-key-original'] as string) ?? msg.fields.routingKey;
      setTimeout(() => {
        try {
          canal.publish(EXCHANGE_EVENTS, routingKey, msg.content, {
            persistent: true,
            headers: { ...msg.properties.headers, 'x-reintentado': true },
          });
        } catch (e) {
          this.logger.error({
            msg: 'Fallo la reinyeccion desde DLQ',
            cola: nombreCola,
            err: e instanceof Error ? e.message : String(e),
          });
        }
      }, espera);
      canal.ack(msg);
      reprocesados += 1;
    }
    return reprocesados;
  }

  /**
   * Mantiene vivo el reintento (AD-04): un poller reinyecta cada cierto
   * intervalo los mensajes de las DLQ de las colas que este servicio declaro.
   */
  activarReintento(intervaloMs = 10_000): void {
    if (this.timerReintento) return;
    this.timerReintento = setInterval(() => {
      for (const cola of this.colasDeclaradas) {
        this.reintentarDesdeDlq(cola).catch((e) =>
          this.logger.error({
            msg: 'Fallo el poller de reintento DLQ',
            cola,
            err: e instanceof Error ? e.message : String(e),
          }),
        );
      }
    }, intervaloMs);
    this.logger.info({ msg: 'Reintento automatico DLQ activo', intervalo_ms: intervaloMs });
  }
}