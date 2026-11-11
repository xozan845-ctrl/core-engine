import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  COLAS,
  ejecutarConContexto,
  Logger,
  PgService,
  RabbitService,
} from '@core/shared';
import type { EventoBus } from '@core/shared';
import { NotificacionesService } from './notificaciones.service';

/**
 * Consumidor de la cola `notifications.todos` (Tabla 22).
 *
 * Reutiliza los eventos ya publicados por la saga; no crea eventos nuevos.
 * Idempotencia por `event_id`: si el envío falla, el mensaje se reintenta y el
 * evento no queda marcado; si ya se procesó, se descarta.
 */
@Injectable()
export class PedidosConsumer implements OnModuleInit {
  private readonly logger = Logger.create('notifications.consumer');

  constructor(
    private readonly rabbit: RabbitService,
    private readonly pg: PgService,
    private readonly notificaciones: NotificacionesService,
  ) {}

  async onModuleInit(): Promise<void> {
    await this.rabbit.declararColas([
      { ...COLAS.notifications_todos, nombre: COLAS.notifications_todos.cola },
    ]);
    await this.rabbit.consumir(COLAS.notifications_todos.cola, (evento) => this.manejar(evento));
    this.rabbit.activarReintento();
    this.logger.info({ msg: 'Consumidor de notificaciones activo' });
  }

  private async manejar(evento: EventoBus): Promise<void> {
    await ejecutarConContexto(
      {
        request_id: `notifications-event-${evento.event_id}`,
        servicio: 'notifications-consumer',
        usuario_id: '00000000-0000-0000-0000-000000000000',
        rol: 'super_admin',
      },
      async () => {
        if (await this.yaProcesado(evento.event_id)) return;
        await this.notificaciones.procesar(evento);
        await this.marcarProcesado(evento);
      },
    );
  }

  private async yaProcesado(eventId: string): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM notifications.eventos_procesados WHERE event_id = $1`,
      [eventId],
    );
    return !!fila;
  }

  private async marcarProcesado(evento: EventoBus): Promise<void> {
    await this.pg.query(
      `INSERT INTO notifications.eventos_procesados (event_id, tipo)
       VALUES ($1, $2) ON CONFLICT (event_id) DO NOTHING`,
      [evento.event_id, evento.tipo],
    );
  }
}
