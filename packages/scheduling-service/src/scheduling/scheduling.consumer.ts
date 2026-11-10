import { Injectable, OnModuleInit } from '@nestjs/common';
import {
  COLAS,
  DomainError,
  EVENTOS,
  EventoBus,
  Logger,
  OrdenData,
  PgService,
  RabbitService,
  ejecutarConContexto,
} from '@core/shared';
import { SchedulingService, OrdenReserva } from './scheduling.service';

@Injectable()
export class SchedulingConsumer implements OnModuleInit {
  private readonly logger = Logger.create('scheduling.consumer');

  constructor(
    private readonly rabbit: RabbitService,
    private readonly pg: PgService,
    private readonly scheduling: SchedulingService,
  ) {}

  async onModuleInit(): Promise<void> {
    await this.rabbit.declararColas([{ ...COLAS.scheduling_orders, nombre: COLAS.scheduling_orders.cola }]);
    await this.rabbit.consumir(COLAS.scheduling_orders.cola, (evento) => this.manejar(evento));
    this.rabbit.activarReintento();
    this.logger.info({ msg: 'Consumidor de reservas activo' });
  }

  private async manejar(evento: EventoBus): Promise<void> {
    await ejecutarConContexto(
      {
        request_id: `scheduling-event-${evento.event_id}`,
        servicio: 'scheduling-consumer',
        usuario_id: '00000000-0000-0000-0000-000000000000',
        rol: 'super_admin',
      },
      async () => {
        if (evento.tipo !== EVENTOS.ORDER_CREATED) return;
        if (await this.yaProcesado(evento.event_id)) return;
        const orden = evento.data as OrdenData & { fulfillment_config?: Record<string, unknown> };
        if (orden.tipo_orden === 'reserva') {
          if (!orden.cliente_parte_id) {
            throw new DomainError('PARTE_REQUERIDA', 'Una reserva requiere cliente_parte_id.');
          }
          await this.scheduling.crearDesdeReserva(orden as OrdenReserva);
        }
        await this.marcarProcesado(evento);
      },
    );
  }

  private async yaProcesado(eventId: string): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM scheduling.eventos_procesados WHERE event_id = $1`,
      [eventId],
    );
    return !!fila;
  }

  private async marcarProcesado(evento: EventoBus): Promise<void> {
    await this.pg.query(
      `INSERT INTO scheduling.eventos_procesados (event_id, tipo, procesado_en)
       VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING`,
      [evento.event_id, evento.tipo],
    );
  }

}
