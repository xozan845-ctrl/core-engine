import { Injectable } from '@nestjs/common';
import { ConflictError, DomainError, NotFoundError, PgService } from '@core/shared';
import { CreateOrderCommand } from './commands/create-order.command';
import { CreateOrderCommandHandler } from './handlers/create-order-command.handler';

/**
 * Suscripciones y ordenes recurrentes (Fase P2.3).
 *
 * Cada vencimiento crea una orden normal usando el precio y stock vigentes de
 * la oferta. La reclamacion de la suscripcion es atomica para que dos workers
 * no generen la misma renovacion.
 */

export type IntervaloSuscripcion = 'semanal' | 'mensual' | 'anual';
export type EstadoSuscripcion = 'activa' | 'pausada' | 'cancelada';
export const INTERVALOS: IntervaloSuscripcion[] = ['semanal', 'mensual', 'anual'];
export const FULFILLMENTS_SUSCRIPCION = ['delivery', 'pickup', 'onsite', 'digital', 'none'] as const;

export interface Suscripcion {
  id: string;
  empresa_id: string;
  oferta_id: string;
  item_id: string | null;
  cantidad: number;
  comprador_id: string;
  cliente_parte_id: string | null;
  tipo_orden: 'compra';
  metodo_pago: 'simulado' | 'credito';
  fulfillment: string;
  intervalo: IntervaloSuscripcion;
  estado: EstadoSuscripcion;
  proxima_ejecucion: string;
  ultima_ejecucion: string | null;
  fin_en: string | null;
  metadata: Record<string, unknown>;
  creado_por: string;
  creado_en: string;
  actualizado_en: string;
}

export interface SuscripcionEjecucion {
  id: string;
  suscripcion_id: string;
  order_id: string | null;
  estado: 'creada' | 'error';
  error: string;
  ejecutada_en: string;
}

export interface CrearSuscripcionInput {
  oferta_id: string;
  item_id?: string | null;
  cantidad?: number;
  comprador_id?: string;
  cliente_parte_id?: string | null;
  metodo_pago?: 'simulado' | 'credito';
  fulfillment?: string;
  intervalo: IntervaloSuscripcion;
  proxima_ejecucion?: string;
  fin_en?: string | null;
  metadata?: Record<string, unknown>;
}

interface SuscripcionRow {
  id: string;
  empresa_id: string;
  oferta_id: string;
  item_id: string | null;
  cantidad: number;
  comprador_id: string;
  cliente_parte_id: string | null;
  tipo_orden: 'compra';
  metodo_pago: 'simulado' | 'credito';
  fulfillment: string;
  intervalo: IntervaloSuscripcion;
  estado: EstadoSuscripcion;
  proxima_ejecucion: string;
  ultima_ejecucion: string | null;
  fin_en: string | null;
  metadata: Record<string, unknown>;
  creado_por: string;
  creado_en: string;
  actualizado_en: string;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const FECHA = /^\d{4}-\d{2}-\d{2}$/;
const COLUMNAS = `id, empresa_id, oferta_id, item_id, cantidad, comprador_id,
  cliente_parte_id, tipo_orden, metodo_pago, fulfillment, intervalo, estado,
  proxima_ejecucion, ultima_ejecucion, fin_en, metadata, creado_por, creado_en,
  actualizado_en`;

@Injectable()
export class SuscripcionesService {
  constructor(
    private readonly pg: PgService,
    private readonly handler: CreateOrderCommandHandler,
  ) {}

  async listar(
    empresaId: string,
    filtros: { estado?: unknown; comprador_id?: unknown; limite?: unknown },
  ): Promise<Suscripcion[]> {
    const params: unknown[] = [empresaId];
    const condiciones = ['empresa_id = $1'];
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`estado = $${params.length}`);
    }
    if (filtros.comprador_id) {
      params.push(String(filtros.comprador_id));
      condiciones.push(`comprador_id = $${params.length}`);
    }
    const limite = Math.min(Math.max(Number(filtros.limite) || 50, 1), 200);
    params.push(limite);
    const filas = await this.pg.query<SuscripcionRow>(
      `SELECT ${COLUMNAS}
         FROM orders.suscripciones
        WHERE ${condiciones.join(' AND ')}
        ORDER BY proxima_ejecucion, creado_en DESC
        LIMIT $${params.length}`,
      params,
    );
    return filas.map((fila) => this.serializar(fila));
  }

  async obtener(empresaId: string, suscripcionId: string): Promise<Suscripcion> {
    return this.serializar(await this.buscar(empresaId, suscripcionId));
  }

  async crear(
    empresaId: string,
    actorId: string,
    input: CrearSuscripcionInput,
  ): Promise<Suscripcion> {
    const ofertaId = this.validarUuid(input.oferta_id, 'oferta');
    const itemId = this.validarUuidOpcional(input.item_id, 'item');
    const cantidad = this.validarCantidad(input.cantidad);
    const compradorId = this.validarUuid(input.comprador_id ?? actorId, 'comprador');
    const clienteParteId = this.validarUuidOpcional(input.cliente_parte_id, 'parte');
    const metodoPago = input.metodo_pago === 'credito' ? 'credito' : 'simulado';
    const fulfillment = this.validarFulfillment(input.fulfillment);
    const intervalo = this.validarIntervalo(input.intervalo);
    const proxima = this.validarFecha(input.proxima_ejecucion, 'proxima_ejecucion')
      ?? new Date().toISOString().slice(0, 10);
    const finEn = this.validarFecha(input.fin_en, 'fin_en');
    if (finEn && finEn < proxima) {
      throw new DomainError('FECHAS_INVALIDAS', 'La fecha fin no puede ser anterior a la proxima ejecucion.');
    }
    if (metodoPago === 'credito' && !clienteParteId) {
      throw new DomainError(
        'CREDITO_REQUIERE_PARTE',
        'Una suscripcion a credito requiere cliente_parte_id.',
      );
    }
    const metadata = this.validarMetadata(input.metadata);

    const fila = await this.pg.queryOne<SuscripcionRow>(
      `INSERT INTO orders.suscripciones
         (empresa_id, oferta_id, item_id, cantidad, comprador_id, cliente_parte_id,
          metodo_pago, fulfillment, intervalo, proxima_ejecucion, fin_en,
          metadata, creado_por)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::date, $11::date,
               $12::jsonb, $13)
       RETURNING ${COLUMNAS}`,
      [
        empresaId,
        ofertaId,
        itemId,
        cantidad,
        compradorId,
        clienteParteId,
        metodoPago,
        fulfillment,
        intervalo,
        proxima,
        finEn,
        JSON.stringify(metadata),
        actorId,
      ],
    );
    if (!fila) throw new DomainError('SUSCRIPCION_NO_CREADA', 'No se pudo crear la suscripcion.');
    return this.serializar(fila);
  }

  async pausar(empresaId: string, suscripcionId: string): Promise<Suscripcion> {
    return this.cambiarEstado(empresaId, suscripcionId, 'pausada');
  }

  async reanudar(empresaId: string, suscripcionId: string): Promise<Suscripcion> {
    return this.cambiarEstado(empresaId, suscripcionId, 'activa');
  }

  async cancelar(empresaId: string, suscripcionId: string): Promise<Suscripcion> {
    return this.cambiarEstado(empresaId, suscripcionId, 'cancelada');
  }

  async renovar(
    empresaId: string,
    suscripcionId: string,
    forzar = true,
  ): Promise<{ suscripcion: Suscripcion; order_id: string }> {
    this.validarUuid(suscripcionId, 'suscripcion');
    const reclamada = await this.pg.queryOne<SuscripcionRow>(
      `UPDATE orders.suscripciones
          SET ultima_ejecucion = CURRENT_DATE,
              proxima_ejecucion = (
                GREATEST(proxima_ejecucion, CURRENT_DATE) +
                CASE intervalo
                  WHEN 'semanal' THEN INTERVAL '7 days'
                  WHEN 'mensual' THEN INTERVAL '1 month'
                  ELSE INTERVAL '1 year'
                END
              )::date,
              actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2 AND estado = 'activa'
          AND ($3::boolean OR proxima_ejecucion <= CURRENT_DATE)
          AND (fin_en IS NULL OR proxima_ejecucion <= fin_en)
        RETURNING ${COLUMNAS}`,
      [suscripcionId, empresaId, forzar],
    );
    if (!reclamada) {
      throw new ConflictError(
        'La suscripcion no esta activa o aun no vence.',
        undefined,
        'SUSCRIPCION_NO_RENOVABLE',
      );
    }

    const comando = {
      tipo_orden: 'compra',
      items: [{
        oferta_id: reclamada.oferta_id,
        cantidad: reclamada.cantidad,
        fulfillment: reclamada.fulfillment,
      }],
      metodo_pago: reclamada.metodo_pago,
      ...(reclamada.cliente_parte_id
        ? { cliente_parte_id: reclamada.cliente_parte_id }
        : {}),
    } as CreateOrderCommand;

    try {
      const vista = await this.handler.ejecutar(comando, reclamada.comprador_id);
      await this.registrarEjecucion(empresaId, reclamada.id, vista.id, 'creada', '');
      return { suscripcion: this.serializar(reclamada), order_id: vista.id };
    } catch (error) {
      await this.pg.query(
        `UPDATE orders.suscripciones
            SET proxima_ejecucion = CURRENT_DATE, actualizado_en = NOW()
          WHERE id = $1 AND empresa_id = $2`,
        [reclamada.id, empresaId],
      );
      await this.registrarEjecucion(
        empresaId,
        reclamada.id,
        null,
        'error',
        (error as Error).message.slice(0, 500),
      );
      throw error;
    }
  }

  async renovarVencidas(): Promise<number> {
    const vencidas = await this.pg.query<{ id: string; empresa_id: string }>(
      `SELECT id, empresa_id
         FROM orders.suscripciones
        WHERE estado = 'activa' AND proxima_ejecucion <= CURRENT_DATE
          AND (fin_en IS NULL OR proxima_ejecucion <= fin_en)
        ORDER BY proxima_ejecucion
        LIMIT 50`,
    );
    let renovadas = 0;
    for (const suscripcion of vencidas) {
      try {
        await this.renovar(suscripcion.empresa_id, suscripcion.id, false);
        renovadas += 1;
      } catch {
        // El error queda en suscripcion_ejecuciones; el siguiente ciclo reintenta.
      }
    }
    return renovadas;
  }

  async listarEjecuciones(
    empresaId: string,
    suscripcionId: string,
  ): Promise<SuscripcionEjecucion[]> {
    await this.buscar(empresaId, suscripcionId);
    return this.pg.query<SuscripcionEjecucion>(
      `SELECT id, suscripcion_id, order_id, estado, error, ejecutada_en
         FROM orders.suscripcion_ejecuciones
        WHERE empresa_id = $1 AND suscripcion_id = $2
        ORDER BY ejecutada_en DESC`,
      [empresaId, suscripcionId],
    );
  }

  // ── helpers ─────────────────────────────────────────────────────────────

  private async cambiarEstado(
    empresaId: string,
    suscripcionId: string,
    estado: EstadoSuscripcion,
  ): Promise<Suscripcion> {
    const actual = await this.buscar(empresaId, suscripcionId);
    const permitidas: Record<EstadoSuscripcion, EstadoSuscripcion[]> = {
      activa: ['pausada', 'cancelada'],
      pausada: ['activa', 'cancelada'],
      cancelada: [],
    };
    if (!permitidas[actual.estado].includes(estado)) {
      throw new ConflictError(`No se puede pasar de ${actual.estado} a ${estado}.`);
    }
    const fila = await this.pg.queryOne<SuscripcionRow>(
      `UPDATE orders.suscripciones
          SET estado = $3, actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2
        RETURNING ${COLUMNAS}`,
      [suscripcionId, empresaId, estado],
    );
    if (!fila) throw new NotFoundError('Suscripcion', suscripcionId);
    return this.serializar(fila);
  }

  private async registrarEjecucion(
    empresaId: string,
    suscripcionId: string,
    orderId: string | null,
    estado: 'creada' | 'error',
    error: string,
  ): Promise<void> {
    await this.pg.query(
      `INSERT INTO orders.suscripcion_ejecuciones
         (empresa_id, suscripcion_id, order_id, estado, error)
       VALUES ($1, $2, $3, $4, $5)`,
      [empresaId, suscripcionId, orderId, estado, error],
    );
  }

  private async buscar(empresaId: string, suscripcionId: string): Promise<SuscripcionRow> {
    this.validarUuid(suscripcionId, 'suscripcion');
    const fila = await this.pg.queryOne<SuscripcionRow>(
      `SELECT ${COLUMNAS}
         FROM orders.suscripciones
        WHERE id = $1 AND empresa_id = $2`,
      [suscripcionId, empresaId],
    );
    if (!fila) throw new NotFoundError('Suscripcion', suscripcionId);
    return fila;
  }

  private serializar(fila: SuscripcionRow): Suscripcion {
    return {
      id: fila.id,
      empresa_id: fila.empresa_id,
      oferta_id: fila.oferta_id,
      item_id: fila.item_id,
      cantidad: fila.cantidad,
      comprador_id: fila.comprador_id,
      cliente_parte_id: fila.cliente_parte_id,
      tipo_orden: 'compra',
      metodo_pago: fila.metodo_pago,
      fulfillment: fila.fulfillment,
      intervalo: fila.intervalo,
      estado: fila.estado,
      proxima_ejecucion: fila.proxima_ejecucion,
      ultima_ejecucion: fila.ultima_ejecucion,
      fin_en: fila.fin_en,
      metadata: fila.metadata ?? {},
      creado_por: fila.creado_por,
      creado_en: fila.creado_en,
      actualizado_en: fila.actualizado_en,
    };
  }

  private validarCantidad(valor: unknown): number {
    const cantidad = valor === undefined ? 1 : Number(valor);
    if (!Number.isInteger(cantidad) || cantidad < 1 || cantidad > 99) {
      throw new DomainError('CANTIDAD_INVALIDA', 'La cantidad debe ser un entero entre 1 y 99.');
    }
    return cantidad;
  }

  private validarIntervalo(valor: unknown): IntervaloSuscripcion {
    const intervalo = String(valor ?? '');
    if (!INTERVALOS.includes(intervalo as IntervaloSuscripcion)) {
      throw new DomainError('INTERVALO_INVALIDO', 'El intervalo debe ser semanal, mensual o anual.');
    }
    return intervalo as IntervaloSuscripcion;
  }

  private validarFulfillment(valor: unknown): string {
    const fulfillment = String(valor ?? 'none');
    if (!FULFILLMENTS_SUSCRIPCION.includes(fulfillment as never)) {
      throw new DomainError('FULFILLMENT_INVALIDO', 'El fulfillment de la suscripcion no es valido.');
    }
    return fulfillment;
  }

  private validarMetadata(valor: unknown): Record<string, unknown> {
    if (valor === undefined || valor === null) return {};
    if (typeof valor !== 'object' || Array.isArray(valor)) {
      throw new DomainError('METADATA_INVALIDA', 'La metadata debe ser un objeto.');
    }
    return valor as Record<string, unknown>;
  }

  private validarFecha(valor: unknown, campo: string): string | null {
    if (valor === undefined || valor === null || valor === '') return null;
    const fecha = String(valor);
    if (!FECHA.test(fecha) || Number.isNaN(new Date(`${fecha}T00:00:00Z`).getTime())) {
      throw new DomainError('FECHA_INVALIDA', `El ${campo} debe ser YYYY-MM-DD.`);
    }
    return fecha;
  }

  private validarUuidOpcional(valor: unknown, campo: string): string | null {
    if (valor === undefined || valor === null || valor === '') return null;
    return this.validarUuid(valor, campo);
  }

  private validarUuid(valor: unknown, campo: string): string {
    const id = String(valor ?? '');
    if (!UUID.test(id)) throw new DomainError('ID_INVALIDO', `El ${campo} debe ser un UUID.`);
    return id;
  }
}
