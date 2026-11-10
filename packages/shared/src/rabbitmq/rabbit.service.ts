import { Injectable, OnModuleDestroy, OnModuleInit, Optional } from '@nestjs/common';
import * as amqp from 'amqplib';
import type { ChannelModel, ConfirmChannel, ConsumeMessage, GetMessage } from 'amqplib';
import { Logger } from '../logging/logger';
import {
  ColaConfig,
  EXCHANGE_DLQ,
  EXCHANGE_EVENTS,
  REINTENTOS_MAXIMOS,
  BACKOFF_BASE_MS,
} from './rabbit.constants';
import { EventoBus } from '../events/contracts';

export interface ColaConNombre extends ColaConfig {
  nombre: string;
}

const RECONEXION_BASE_MS = 1_000;
const RECONEXION_MAX_MS = 30_000;

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
  private timerReconexion?: NodeJS.Timeout;
  private intentosReconexion = 0;
  private detenido = false;

  constructor(@Optional() private readonly url?: string) {
    this.url = this.url ?? process.env.RABBITMQ_URL ?? '';
  }

  async onModuleInit(): Promise<void> {
    await this.conectar();
  }

  async onModuleDestroy(): Promise<void> {
    this.detenido = true;
    if (this.timerReintento) {
      clearInterval(this.timerReintento);
      this.timerReintento = undefined;
    }
    if (this.timerReconexion) {
      clearTimeout(this.timerReconexion);
      this.timerReconexion = undefined;
    }
    const canal = this.canal;
    const conexion = this.conexion;
    this.canal = undefined;
    this.conexion = undefined;
    try {
      if (canal) await canal.close();
      if (conexion) await conexion.close();
    } catch {
      /* cierre en frio */
    }
  }

  private async conectar(): Promise<void> {
    if (this.detenido) return;
    if (!this.url) {
      throw new Error('RABBITMQ_URL no esta definida.');
    }
    // amqplib 2 lee el heartbeat de la query de la URL (Options.Connect), no de
    // las opciones de socket: se declara aca para conservar la negociacion de
    // 30s sin pisar un valor explicito de RABBITMQ_URL.
    const url = /[?&]heartbeat=/.test(this.url)
      ? this.url
      : `${this.url}${this.url.includes('?') ? '&' : '?'}heartbeat=30`;
    const conexion = await amqp.connect(url);
    if (this.detenido) {
      try {
        await conexion.close();
      } catch {
        /* conexion ya cerrada */
      }
      return;
    }
    try {
      const canal = await conexion.createConfirmChannel();
      await canal.assertExchange(EXCHANGE_EVENTS, 'topic', { durable: true });
      await canal.assertExchange(EXCHANGE_DLQ, 'topic', { durable: true });
      if (this.detenido) {
        await conexion.close();
        return;
      }
      // Los listeners se instalan despues del setup: si el arranque falla no
      // se programa una reconexion para una conexion que nunca quedo activa.
      conexion.on('error', (err) =>
        this.logger.error({ msg: 'Error en conexion RabbitMQ', err: err.message }),
      );
      conexion.on('close', () => {
        // Un evento 'close' tardio de una conexion vieja no debe borrar la
        // conexion nueva que ya tomo el relevo.
        if (this.conexion === conexion) this.alCerrarConexion();
      });
      this.conexion = conexion;
      this.canal = canal;
      this.intentosReconexion = 0;
      this.logger.info({ msg: 'Broker conectado', exchange: EXCHANGE_EVENTS });
    } catch (error) {
      try {
        await conexion.close();
      } catch {
        /* conexion ya cerrada */
      }
      throw error;
    }
  }

  private alCerrarConexion(): void {
    this.canal = undefined;
    this.conexion = undefined;
    if (this.detenido) return;
    const espera = this.esperaReconexion();
    this.logger.warn({ msg: 'Conexion al broker cerrada; reintentando', espera_ms: espera });
    this.programarReconexion(espera);
  }

  private esperaReconexion(): number {
    return Math.min(
      RECONEXION_MAX_MS,
      RECONEXION_BASE_MS * 2 ** Math.min(this.intentosReconexion, 5),
    );
  }

  private programarReconexion(espera = this.esperaReconexion()): void {
    if (this.detenido || this.timerReconexion) return;
    this.intentosReconexion += 1;
    this.timerReconexion = setTimeout(() => {
      this.timerReconexion = undefined;
      void this.conectar().catch((error) => {
        this.logger.error({
          msg: 'Fallo al reconectar con RabbitMQ',
          err: error instanceof Error ? error.message : String(error),
        });
        this.programarReconexion();
      });
    }, espera);
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
    const headers = evento.request_id ? { 'x-request-id': evento.request_id } : undefined;
    const ok = canal.publish(
      EXCHANGE_EVENTS,
      evento.tipo,
      Buffer.from(JSON.stringify(evento)),
      { persistent: true, contentType: 'application/json', headers },
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
      const intentos = (msg.properties.headers?.['x-intentos'] ?? 0) as number;
      let cuerpo: EventoBus;
      try {
        cuerpo = JSON.parse(msg.content.toString()) as EventoBus;
      } catch (err) {
        const errMsg = err instanceof Error ? err.message : String(err);
        this.logger.error({
          msg: 'Mensaje con JSON invalido; se envia a DLQ',
          cola: nombreCola,
          err: errMsg,
        });
        await this.deadLetter(canal, nombreCola, msg, REINTENTOS_MAXIMOS, `JSON invalido: ${errMsg}`, true);
        return;
      }
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
        await this.deadLetter(
          canal,
          nombreCola,
          msg,
          siguiente,
          errMsg,
          siguiente > REINTENTOS_MAXIMOS,
        );
      }
    });
  }

  /**
   * Publica el mensaje en la DLQ y solo entonces confirma el original. Si la
   * publicacion no se confirma, devuelve el mensaje a su cola para no perderlo.
   */
  private async deadLetter(
    canal: ConfirmChannel,
    nombreCola: string,
    msg: ConsumeMessage | GetMessage,
    intentos: number,
    error: string,
    muerto: boolean,
  ): Promise<void> {
    const headers = {
      ...msg.properties.headers,
      'x-intentos': intentos,
      'x-error': error,
      'x-cola': nombreCola,
      'x-routing-key-original': msg.fields.routingKey,
      ...(muerto ? { 'x-muerto': true } : {}),
    };
    try {
      canal.publish(EXCHANGE_DLQ, nombreCola, msg.content, {
        persistent: true,
        headers,
      });
      await canal.waitForConfirms();
      canal.ack(msg);
    } catch (dlqError) {
      this.logger.error({
        msg: 'Fallo al publicar en DLQ; se devuelve el mensaje a la cola',
        cola: nombreCola,
        err: dlqError instanceof Error ? dlqError.message : String(dlqError),
      });
      try {
        // Los mensajes muertos no se reencolan: un mensaje corrupto que no
        // puede llegar a la DLQ bloquearía la cola principal en bucle.
        canal.nack(msg, false, !muerto);
      } catch (nackError) {
        this.logger.error({
          msg: 'Fallo al devolver el mensaje a la cola',
          cola: nombreCola,
          err: nackError instanceof Error ? nackError.message : String(nackError),
        });
      }
    }
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
      if (intentos >= REINTENTOS_MAXIMOS) {
        this.logger.error({
          msg: 'Mensaje descartado (muerto) tras maximos intentos',
          cola: nombreCola,
          routing_key: msg.fields.routingKey,
        });
        canal.ack(msg);
        continue;
      }
      const espera = Math.min(30_000, BACKOFF_BASE_MS * 2 ** Math.min(intentos, 10));
      await this.esperar(espera);
      const routingKey =
        (msg.properties.headers?.['x-routing-key-original'] as string) ?? msg.fields.routingKey;
      try {
        canal.publish(EXCHANGE_EVENTS, routingKey, msg.content, {
          persistent: true,
          headers: { ...msg.properties.headers, 'x-reintentado': true },
        });
        await canal.waitForConfirms();
        canal.ack(msg);
        reprocesados += 1;
      } catch (error) {
        this.logger.error({
          msg: 'Fallo la reinyeccion desde DLQ; se conserva en la cola',
          cola: nombreCola,
          err: error instanceof Error ? error.message : String(error),
        });
        try {
          canal.nack(msg, false, true);
        } catch (nackError) {
          this.logger.error({
            msg: 'Fallo al devolver el mensaje de DLQ',
            cola: nombreCola,
            err: nackError instanceof Error ? nackError.message : String(nackError),
          });
        }
        break;
      }
    }
    return reprocesados;
  }

  private esperar(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
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
