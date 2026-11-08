import { Injectable, OnModuleDestroy, OnModuleInit } from '@nestjs/common';
import { randomUUID } from 'node:crypto';
import type { PoolClient } from 'pg';
import {
  AppointmentCancelledData,
  AppointmentCreatedData,
  AppointmentReminderDueData,
  ConflictError,
  DomainError,
  EVENTOS,
  Logger,
  NotFoundError,
  OutboxService,
  PgService,
  ejecutarConContexto,
} from '@core/shared';
import {
  CrearBloqueoRequestDto,
  CrearCitaRequestDto,
  CrearDisponibilidadRequestDto,
  CrearRecursoRequestDto,
} from './scheduling.dtos';

type Fila = Record<string, unknown>;
type OrdenReserva = {
  order_id: string;
  cliente_id: string;
  items: Array<{
    empresa_id: string;
    item_type?: string;
    item_id?: string;
    fulfillment?: string;
    fulfillment_config?: Record<string, unknown>;
  }>;
};

const ESTADOS_ACTIVOS = ['reservada', 'confirmada', 'en_atencion'];

@Injectable()
export class SchedulingService implements OnModuleInit, OnModuleDestroy {
  private readonly logger = Logger.create('scheduling');
  private timer?: NodeJS.Timeout;

  constructor(
    private readonly pg: PgService,
    private readonly outbox: OutboxService,
  ) {}

  onModuleInit(): void {
    this.timer = setInterval(() => {
      void (ejecutarConContexto(
        {
          request_id: `scheduling-worker-${Date.now()}`,
          servicio: 'scheduling-worker',
          usuario_id: '00000000-0000-0000-0000-000000000000',
          rol: 'super_admin',
        },
        () => this.procesarRecordatorios(),
      ) as Promise<void>).catch((error: unknown) => {
        this.logger.error({ msg: 'Fallo al procesar recordatorios', err: error instanceof Error ? error.message : String(error) });
      });
    }, 60_000);
  }

  onModuleDestroy(): void {
    if (this.timer) clearInterval(this.timer);
  }

  async crearRecurso(empresaId: string, dto: CrearRecursoRequestDto): Promise<Fila> {
    const fila = await this.pg.queryOne<Fila>(
      `INSERT INTO scheduling.recursos (id, empresa_id, tipo, nombre, atributos)
       VALUES ($1, $2, $3, $4, $5::jsonb) RETURNING *`,
      [randomUUID(), empresaId, dto.tipo, dto.nombre.trim(), JSON.stringify(dto.atributos ?? {})],
    );
    if (!fila) throw new DomainError('PERSISTENCIA_FALLIDA', 'No se pudo crear el recurso.');
    return fila;
  }

  listarRecursos(empresaId: string): Promise<Fila[]> {
    return this.pg.query<Fila>(
      `SELECT * FROM scheduling.recursos
       WHERE empresa_id = $1 ORDER BY estado ASC, nombre ASC`,
      [empresaId],
    );
  }

  async crearDisponibilidad(empresaId: string, dto: CrearDisponibilidadRequestDto): Promise<Fila> {
    await this.recursoDeEmpresa(empresaId, dto.recurso_id);
    const fila = await this.pg.queryOne<Fila>(
      `INSERT INTO scheduling.disponibilidad
        (id, empresa_id, recurso_id, dia_semana, inicia_hora, termina_hora,
         zona_horaria, vigente_desde, vigente_hasta)
       VALUES ($1, $2, $3, $4, $5, $6, $7, COALESCE($8::date, CURRENT_DATE), $9::date)
       RETURNING *`,
      [randomUUID(), empresaId, dto.recurso_id, dto.dia_semana, dto.inicia_hora,
        dto.termina_hora, dto.zona_horaria ?? 'America/Managua', dto.vigente_desde ?? null, dto.vigente_hasta ?? null],
    );
    if (!fila) throw new DomainError('PERSISTENCIA_FALLIDA', 'No se pudo guardar la disponibilidad.');
    return fila;
  }

  listarDisponibilidad(empresaId: string, recursoId: string): Promise<Fila[]> {
    return this.pg.query<Fila>(
      `SELECT * FROM scheduling.disponibilidad
       WHERE empresa_id = $1 AND recurso_id = $2
       ORDER BY dia_semana, inicia_hora`,
      [empresaId, recursoId],
    );
  }

  async crearBloqueo(empresaId: string, actorId: string | undefined, dto: CrearBloqueoRequestDto): Promise<Fila> {
    this.validarPeriodo(dto.inicia_en, dto.termina_en);
    await this.recursoDeEmpresa(empresaId, dto.recurso_id);
    try {
      const fila = await this.pg.queryOne<Fila>(
        `INSERT INTO scheduling.bloqueos
          (id, empresa_id, recurso_id, periodo, motivo, creado_por)
         VALUES ($1, $2, $3, tstzrange($4::timestamptz, $5::timestamptz, '[)'), $6, $7)
         RETURNING id, empresa_id, recurso_id, lower(periodo) AS inicia_en,
                   upper(periodo) AS termina_en, motivo, creado_en`,
        [randomUUID(), empresaId, dto.recurso_id, dto.inicia_en, dto.termina_en, dto.motivo ?? '', actorId ?? null],
      );
      if (!fila) throw new DomainError('PERSISTENCIA_FALLIDA', 'No se pudo crear el bloqueo.');
      return fila;
    } catch (error) {
      this.traducirConflictoPeriodo(error, 'El recurso ya tiene un bloqueo en ese horario.');
    }
  }

  async crearCita(empresaId: string, dto: CrearCitaRequestDto): Promise<Fila> {
    this.validarPeriodo(dto.inicia_en, dto.termina_en);
    try {
      return await this.pg.transaccion(async (client) => {
        const recurso = await client.query<Fila>(
          `SELECT id FROM scheduling.recursos
           WHERE id = $1 AND empresa_id = $2 AND estado = 'activo' FOR UPDATE`,
          [dto.recurso_id, empresaId],
        );
        if (recurso.rowCount === 0) throw new NotFoundError('Recurso', dto.recurso_id);
        await this.validarDisponibilidad(client, dto.recurso_id, dto.inicia_en, dto.termina_en);
        const bloqueo = await client.query(
          `SELECT 1 FROM scheduling.bloqueos
           WHERE recurso_id = $1
             AND periodo && tstzrange($2::timestamptz, $3::timestamptz, '[)') LIMIT 1`,
          [dto.recurso_id, dto.inicia_en, dto.termina_en],
        );
        if ((bloqueo.rowCount ?? 0) > 0) {
          throw new ConflictError('El recurso está bloqueado en ese horario.', undefined, 'RECURSO_NO_DISPONIBLE');
        }
        const id = randomUUID();
        const result = await client.query<Fila>(
          `INSERT INTO scheduling.citas
            (id, empresa_id, recurso_id, cliente_id, order_id, item_id,
             inicia_en, termina_en, notas, metadata)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::jsonb)
           RETURNING id, empresa_id, recurso_id, cliente_id, order_id, item_id,
                     estado, inicia_en, termina_en, notas, metadata, creado_en, actualizado_en`,
          [id, empresaId, dto.recurso_id, dto.cliente_id ?? null, dto.order_id ?? null,
            dto.item_id ?? null, dto.inicia_en, dto.termina_en, dto.notas ?? '', JSON.stringify(dto.metadata ?? {})],
        );
        const cita = result.rows[0];
        if (!cita) throw new DomainError('PERSISTENCIA_FALLIDA', 'No se pudo crear la cita.');
        await this.crearRecordatorios(client, empresaId, id, dto.inicia_en);
        await this.outbox.insertarEnTransaccion(client, EVENTOS.APPOINTMENT_CREATED, {
          cita_id: String(cita.id),
          empresa_id: empresaId,
          recurso_id: String(cita.recurso_id),
          ...(cita.cliente_id ? { cliente_id: String(cita.cliente_id) } : {}),
          ...(cita.order_id ? { order_id: String(cita.order_id) } : {}),
          ...(cita.item_id ? { item_id: String(cita.item_id) } : {}),
          inicia_en: String(cita.inicia_en),
          termina_en: String(cita.termina_en),
          estado: String(cita.estado) as 'reservada' | 'confirmada',
        } satisfies AppointmentCreatedData);
        return cita;
      });
    } catch (error) {
      this.traducirConflictoPeriodo(error, 'El recurso ya está reservado en ese horario.');
    }
  }

  listarCitas(empresaId: string, desde?: string, hasta?: string): Promise<Fila[]> {
    return this.pg.query<Fila>(
      `SELECT id, empresa_id, recurso_id, cliente_id, order_id, item_id, estado,
              inicia_en, termina_en, notas, metadata, creado_en, actualizado_en
       FROM scheduling.citas
       WHERE empresa_id = $1
         AND ($2::timestamptz IS NULL OR termina_en > $2::timestamptz)
         AND ($3::timestamptz IS NULL OR inicia_en < $3::timestamptz)
       ORDER BY inicia_en ASC LIMIT 500`,
      [empresaId, desde ?? null, hasta ?? null],
    );
  }

  async cancelarCita(empresaId: string, citaId: string, motivo?: string): Promise<Fila> {
    return this.pg.transaccion(async (client) => {
      const result = await client.query<Fila>(
        `UPDATE scheduling.citas
            SET estado = 'cancelada', actualizado_en = NOW()
          WHERE id = $1 AND empresa_id = $2
            AND estado = ANY($3::text[])
        RETURNING id, empresa_id, recurso_id, cliente_id, order_id, item_id, estado,
                  inicia_en, termina_en, notas, metadata, creado_en, actualizado_en`,
        [citaId, empresaId, ESTADOS_ACTIVOS],
      );
      const cita = result.rows[0];
      if (!cita) throw new NotFoundError('Cita activa', citaId);
      await client.query(
        `UPDATE scheduling.recordatorios SET estado = 'cancelado'
         WHERE cita_id = $1 AND estado = 'pendiente'`,
        [citaId],
      );
      await this.outbox.insertarEnTransaccion(client, EVENTOS.APPOINTMENT_CANCELLED, {
        cita_id: String(cita.id),
        empresa_id: empresaId,
        ...(cita.order_id ? { order_id: String(cita.order_id) } : {}),
        ...(motivo ? { motivo } : {}),
      } satisfies AppointmentCancelledData);
      return cita;
    });
  }

  listarRecordatorios(empresaId: string): Promise<Fila[]> {
    return this.pg.query<Fila>(
      `SELECT id, cita_id, tipo, programado_en, estado, enviado_en
       FROM scheduling.recordatorios
       WHERE empresa_id = $1 ORDER BY programado_en ASC LIMIT 500`,
      [empresaId],
    );
  }

  async crearDesdeReserva(orden: OrdenReserva): Promise<Fila | null> {
    const existente = await this.pg.queryOne<Fila>(
      `SELECT id, empresa_id, recurso_id, cliente_id, order_id, item_id, estado,
              inicia_en, termina_en, notas, metadata, creado_en, actualizado_en
       FROM scheduling.citas WHERE order_id = $1`,
      [orden.order_id],
    );
    if (existente) return existente;
    const item = orden.items.find((linea) => linea.item_type === 'servicio' && linea.fulfillment === 'onsite');
    const config = item?.fulfillment_config ?? {};
    const recursoId = this.textoConfig(config, 'recurso_id');
    const iniciaEn = this.textoConfig(config, 'inicia_en');
    const terminaEn = this.textoConfig(config, 'termina_en');
    if (!item || !recursoId || !iniciaEn || !terminaEn) {
      throw new DomainError(
        'CITA_CONFIGURACION_REQUERIDA',
        'Una reserva requiere un servicio onsite con recurso_id, inicia_en y termina_en.',
      );
    }
    return this.crearCita(item.empresa_id, {
      recurso_id: recursoId,
      cliente_id: orden.cliente_id,
      order_id: orden.order_id,
      item_id: item.item_id,
      inicia_en: iniciaEn,
      termina_en: terminaEn,
      notas: this.textoConfig(config, 'notas'),
      metadata: { origen: 'order.created', ...(config.metadata && typeof config.metadata === 'object' ? { reserva: config.metadata } : {}) },
    });
  }

  async procesarRecordatorios(): Promise<number> {
    return this.pg.transaccion(async (client) => {
      const result = await client.query<Fila>(
        `SELECT r.id, r.cita_id, r.empresa_id, r.tipo, r.programado_en
         FROM scheduling.recordatorios r
         WHERE r.estado = 'pendiente' AND r.programado_en <= NOW()
         ORDER BY r.programado_en ASC LIMIT 100 FOR UPDATE SKIP LOCKED`,
      );
      for (const recordatorio of result.rows) {
        await client.query(
          `UPDATE scheduling.recordatorios
              SET estado = 'enviado', enviado_en = NOW()
            WHERE id = $1 AND estado = 'pendiente'`,
          [recordatorio.id],
        );
        await this.outbox.insertarEnTransaccion(client, EVENTOS.APPOINTMENT_REMINDER_DUE, {
          recordatorio_id: String(recordatorio.id),
          cita_id: String(recordatorio.cita_id),
          empresa_id: String(recordatorio.empresa_id),
          tipo: String(recordatorio.tipo) as '24h' | '2h',
          programado_en: String(recordatorio.programado_en),
        } satisfies AppointmentReminderDueData);
      }
      return result.rows.length;
    });
  }

  private async recursoDeEmpresa(empresaId: string, recursoId: string): Promise<void> {
    const recurso = await this.pg.queryOne(
      `SELECT id FROM scheduling.recursos WHERE id = $1 AND empresa_id = $2`,
      [recursoId, empresaId],
    );
    if (!recurso) throw new NotFoundError('Recurso', recursoId);
  }

  private async validarDisponibilidad(client: PoolClient, recursoId: string, inicia: string, termina: string): Promise<void> {
    const disponible = await client.query(
      `SELECT 1 FROM scheduling.disponibilidad d
       WHERE d.recurso_id = $1
         AND d.dia_semana = EXTRACT(DOW FROM ($2::timestamptz AT TIME ZONE d.zona_horaria))::int
         AND d.inicia_hora <= (($2::timestamptz AT TIME ZONE d.zona_horaria)::time)
         AND d.termina_hora >= (($3::timestamptz AT TIME ZONE d.zona_horaria)::time)
         AND d.vigente_desde <= (($2::timestamptz AT TIME ZONE d.zona_horaria)::date)
         AND (d.vigente_hasta IS NULL OR d.vigente_hasta >= (($2::timestamptz AT TIME ZONE d.zona_horaria)::date))
       LIMIT 1`,
      [recursoId, inicia, termina],
    );
    if (disponible.rowCount === 0) {
      throw new ConflictError('El recurso no tiene disponibilidad en ese horario.', undefined, 'RECURSO_NO_DISPONIBLE');
    }
  }

  private async crearRecordatorios(client: PoolClient, empresaId: string, citaId: string, inicia: string): Promise<void> {
    await client.query(
      `INSERT INTO scheduling.recordatorios (id, empresa_id, cita_id, tipo, programado_en)
       SELECT gen_random_uuid(), $1, $2, tipo, ($3::timestamptz - intervalo)
       FROM (VALUES ('24h'::text, INTERVAL '24 hours'), ('2h'::text, INTERVAL '2 hours')) AS x(tipo, intervalo)
       WHERE ($3::timestamptz - intervalo) > NOW()
       ON CONFLICT (cita_id, tipo) DO NOTHING`,
      [empresaId, citaId, inicia],
    );
  }

  private validarPeriodo(inicia: string, termina: string): void {
    const inicioMs = Date.parse(inicia);
    const finMs = Date.parse(termina);
    if (!Number.isFinite(inicioMs) || !Number.isFinite(finMs) || inicioMs >= finMs) {
      throw new DomainError('PERIODO_INVALIDO', 'La hora de inicio debe ser anterior a la hora de finalizacion.');
    }
  }

  private traducirConflictoPeriodo(error: unknown, mensaje: string): never {
    if (typeof error === 'object' && error !== null && 'code' in error && (error as { code?: string }).code === '23P01') {
      throw new ConflictError(mensaje, undefined, 'RECURSO_NO_DISPONIBLE');
    }
    throw error;
  }

  private textoConfig(config: Record<string, unknown>, clave: string): string | undefined {
    const valor = config[clave];
    return typeof valor === 'string' && valor.trim() ? valor : undefined;
  }
}
