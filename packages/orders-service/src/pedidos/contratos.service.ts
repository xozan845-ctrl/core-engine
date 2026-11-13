import { Injectable } from '@nestjs/common';
import { ConflictError, DomainError, Money, NotFoundError, PgService } from '@core/shared';

/**
 * Contratos con deposito/garantia (Fase P2.2).
 *
 * Sirve para alquileres, servicios y acuerdos que retienen un deposito. El
 * contrato puede vincularse a una orden y conserva su historial de movimientos.
 */

export type TipoContrato = 'alquiler' | 'servicio' | 'compra' | 'otro';
export type EstadoContrato = 'borrador' | 'activo' | 'finalizado' | 'cancelado';
export const TIPOS_CONTRATO: TipoContrato[] = ['alquiler', 'servicio', 'compra', 'otro'];

export interface Contrato {
  id: string;
  empresa_id: string;
  order_id: string | null;
  parte_id: string | null;
  item_id: string | null;
  tipo: TipoContrato;
  titulo: string;
  descripcion: string;
  estado: EstadoContrato;
  inicia_en: string | null;
  termina_en: string | null;
  deposito: string;
  garantia: string;
  retenido: string;
  moneda: string;
  metadata: Record<string, unknown>;
  creado_por: string;
  creado_en: string;
  actualizado_en: string;
  finalizado_en: string | null;
}

export interface ContratoEvento {
  id: string;
  contrato_id: string;
  tipo: string;
  monto: string;
  motivo: string;
  creado_por: string;
  creado_en: string;
}

export interface CrearContratoInput {
  order_id?: string | null;
  parte_id?: string | null;
  item_id?: string | null;
  tipo?: TipoContrato;
  titulo: string;
  descripcion?: string;
  inicia_en?: string | null;
  termina_en?: string | null;
  deposito?: string | number;
  garantia?: string | number;
  moneda?: string;
  metadata?: Record<string, unknown>;
}

export type ActualizarContratoInput = Partial<Omit<CrearContratoInput, 'order_id'>> & {
  estado?: 'activo' | 'cancelado';
};

interface ContratoRow {
  id: string;
  empresa_id: string;
  order_id: string | null;
  parte_id: string | null;
  item_id: string | null;
  tipo: TipoContrato;
  titulo: string;
  descripcion: string;
  estado: EstadoContrato;
  inicia_en: string | null;
  termina_en: string | null;
  deposito_cents: number;
  garantia_cents: number;
  retenido_cents: number;
  moneda: string;
  metadata: Record<string, unknown>;
  creado_por: string;
  creado_en: string;
  actualizado_en: string;
  finalizado_en: string | null;
}

interface EventoRow {
  id: string;
  contrato_id: string;
  tipo: string;
  monto_cents: number;
  motivo: string;
  creado_por: string;
  creado_en: string;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const FECHA = /^\d{4}-\d{2}-\d{2}$/;
const MONEDA = /^[A-Z]{3}$/;
const COLUMNAS = `id, empresa_id, order_id, parte_id, item_id, tipo, titulo,
  descripcion, estado, inicia_en, termina_en, deposito_cents, garantia_cents,
  retenido_cents, moneda, metadata, creado_por, creado_en, actualizado_en,
  finalizado_en`;

@Injectable()
export class ContratosService {
  constructor(private readonly pg: PgService) {}

  async listar(
    empresaId: string,
    filtros: { estado?: unknown; order_id?: unknown; parte_id?: unknown; pagina?: unknown; limite?: unknown },
  ): Promise<Contrato[]> {
    const params: unknown[] = [empresaId];
    const condiciones = ['empresa_id = $1'];
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`estado = $${params.length}`);
    }
    if (filtros.order_id) {
      params.push(String(filtros.order_id));
      condiciones.push(`order_id = $${params.length}`);
    }
    if (filtros.parte_id) {
      params.push(String(filtros.parte_id));
      condiciones.push(`parte_id = $${params.length}`);
    }
    const limite = Math.min(Math.max(Number(filtros.limite) || 50, 1), 200);
    const pagina = Math.max(Number(filtros.pagina) || 1, 1);
    params.push(limite, (pagina - 1) * limite);
    const filas = await this.pg.query<ContratoRow>(
      `SELECT ${COLUMNAS}
         FROM orders.contratos
        WHERE ${condiciones.join(' AND ')}
        ORDER BY creado_en DESC
        LIMIT $${params.length - 1} OFFSET $${params.length}`,
      params,
    );
    return filas.map((fila) => this.serializar(fila));
  }

  async obtener(empresaId: string, contratoId: string): Promise<Contrato> {
    return this.serializar(await this.buscar(empresaId, contratoId));
  }

  async crear(
    empresaId: string,
    actorId: string,
    input: CrearContratoInput,
  ): Promise<Contrato> {
    const titulo = this.validarTitulo(input.titulo);
    const tipo = this.validarTipo(input.tipo);
    const descripcion = String(input.descripcion ?? '').trim().slice(0, 2000);
    const { inicia, termina } = this.validarFechas(input.inicia_en, input.termina_en);
    const deposito = Money.parsear(input.deposito ?? '0');
    const garantia = Money.parsear(input.garantia ?? '0');
    const moneda = this.validarMoneda(input.moneda);
    const metadata = this.validarMetadata(input.metadata);
    const orderId = this.validarUuidOpcional(input.order_id, 'orden');
    const parteId = this.validarUuidOpcional(input.parte_id, 'parte');
    const itemId = this.validarUuidOpcional(input.item_id, 'item');

    let parteFinal = parteId;
    if (orderId) {
      const orden = await this.pg.queryOne<{ estado: string; parte_id: string | null }>(
        `SELECT estado, parte_id FROM orders.orden_vista WHERE id = $1`,
        [orderId],
      );
      if (!orden) throw new NotFoundError('Orden', orderId);
      if (!parteFinal && orden.parte_id) parteFinal = orden.parte_id;
    }

    const fila = await this.pg.queryOne<ContratoRow>(
      `INSERT INTO orders.contratos
         (empresa_id, order_id, parte_id, item_id, tipo, titulo, descripcion,
          inicia_en, termina_en, deposito_cents, garantia_cents, moneda,
          metadata, creado_por)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8::date, $9::date, $10, $11, $12,
               $13::jsonb, $14)
       RETURNING ${COLUMNAS}`,
      [
        empresaId,
        orderId,
        parteFinal,
        itemId,
        tipo,
        titulo,
        descripcion,
        inicia,
        termina,
        deposito.centavos,
        garantia.centavos,
        moneda,
        JSON.stringify(metadata),
        actorId,
      ],
    );
    if (!fila) throw new DomainError('CONTRATO_NO_CREADO', 'No se pudo crear el contrato.');
    return this.serializar(fila);
  }

  async actualizar(
    empresaId: string,
    contratoId: string,
    input: ActualizarContratoInput,
  ): Promise<Contrato> {
    const actual = await this.buscar(empresaId, contratoId);
    if (actual.estado === 'finalizado' || actual.estado === 'cancelado') {
      throw new ConflictError('El contrato ya esta cerrado.');
    }
    const estado = input.estado ?? actual.estado;
    this.validarTransicion(actual.estado, estado);
    const titulo = input.titulo === undefined ? actual.titulo : this.validarTitulo(input.titulo);
    const tipo = input.tipo === undefined ? actual.tipo : this.validarTipo(input.tipo);
    const descripcion = input.descripcion === undefined
      ? actual.descripcion
      : String(input.descripcion).trim().slice(0, 2000);
    const { inicia, termina } = this.validarFechas(
      input.inicia_en === undefined ? actual.inicia_en : input.inicia_en,
      input.termina_en === undefined ? actual.termina_en : input.termina_en,
    );
    const deposito = input.deposito === undefined
      ? Money.desdeCentavos(actual.deposito_cents)
      : Money.parsear(input.deposito);
    if (actual.retenido_cents > deposito.centavos) {
      throw new DomainError('MONTO_INVALIDO', 'El deposito no puede ser menor que lo retenido.');
    }
    const garantia = input.garantia === undefined
      ? Money.desdeCentavos(actual.garantia_cents)
      : Money.parsear(input.garantia);
    const moneda = input.moneda === undefined ? actual.moneda : this.validarMoneda(input.moneda);
    const metadata = input.metadata === undefined ? actual.metadata : this.validarMetadata(input.metadata);

    return this.pg.transaccion(async (client) => {
      const fila = await client.query<ContratoRow>(
        `UPDATE orders.contratos
            SET titulo = $2, tipo = $3, descripcion = $4, estado = $5,
                inicia_en = $6::date, termina_en = $7::date,
                deposito_cents = $8, garantia_cents = $9, moneda = $10,
                metadata = $11::jsonb, actualizado_en = NOW()
          WHERE id = $1 AND empresa_id = $12
          RETURNING ${COLUMNAS}`,
        [
          contratoId,
          titulo,
          tipo,
          descripcion,
          estado,
          inicia,
          termina,
          deposito.centavos,
          garantia.centavos,
          moneda,
          JSON.stringify(metadata),
          empresaId,
        ],
      );
      const contrato = fila.rows[0];
      if (!contrato) throw new NotFoundError('Contrato', contratoId);
      if (estado === 'activo' && actual.estado !== 'activo' && deposito.centavos > 0) {
        await this.registrarEvento(
          client,
          empresaId,
          contratoId,
          'deposito_recibido',
          deposito.centavos,
          'Deposito al activar el contrato',
          actual.creado_por,
        );
      }
      if (estado === 'cancelado' && actual.estado !== 'cancelado') {
        await this.registrarEvento(
          client,
          empresaId,
          contratoId,
          'cancelacion',
          0,
          'Contrato cancelado',
          actual.creado_por,
        );
        if (actual.estado === 'activo' && actual.deposito_cents > 0) {
          await this.registrarEvento(
            client,
            empresaId,
            contratoId,
            'deposito_devuelto',
            actual.deposito_cents,
            'Devolucion por cancelacion',
            actual.creado_por,
          );
        }
      }
      return this.serializar(contrato);
    });
  }

  async finalizar(
    empresaId: string,
    contratoId: string,
    actorId: string,
    input: { retenido?: string | number; motivo?: string },
  ): Promise<Contrato> {
    const actual = await this.buscar(empresaId, contratoId);
    if (actual.estado !== 'activo') {
      throw new ConflictError('Solo un contrato activo puede finalizarse.');
    }
    const retenido = Money.parsear(input.retenido ?? '0');
    if (retenido.centavos > actual.deposito_cents) {
      throw new DomainError('MONTO_INVALIDO', 'Lo retenido no puede superar el deposito.');
    }
    const devuelto = actual.deposito_cents - retenido.centavos;
    const motivo = String(input.motivo ?? '').trim().slice(0, 500);

    return this.pg.transaccion(async (client) => {
      const fila = await client.query<ContratoRow>(
        `UPDATE orders.contratos
            SET estado = 'finalizado', retenido_cents = $3, finalizado_en = NOW(),
                actualizado_en = NOW()
          WHERE id = $1 AND empresa_id = $2 AND estado = 'activo'
          RETURNING ${COLUMNAS}`,
        [contratoId, empresaId, retenido.centavos],
      );
      const contrato = fila.rows[0];
      if (!contrato) throw new ConflictError('El contrato ya no esta activo.');
      if (retenido.centavos > 0) {
        await this.registrarEvento(
          client, empresaId, contratoId, 'retencion', retenido.centavos, motivo, actorId,
        );
      }
      if (devuelto > 0) {
        await this.registrarEvento(
          client, empresaId, contratoId, 'deposito_devuelto', devuelto, motivo, actorId,
        );
      }
      return this.serializar(contrato);
    });
  }

  async listarEventos(empresaId: string, contratoId: string): Promise<ContratoEvento[]> {
    await this.buscar(empresaId, contratoId);
    const filas = await this.pg.query<EventoRow>(
      `SELECT id, contrato_id, tipo, monto_cents, motivo, creado_por, creado_en
         FROM orders.contrato_eventos
        WHERE empresa_id = $1 AND contrato_id = $2
        ORDER BY creado_en, id`,
      [empresaId, contratoId],
    );
    return filas.map((fila) => this.serializarEvento(fila));
  }

  // ── helpers ─────────────────────────────────────────────────────────────

  private async buscar(empresaId: string, contratoId: string): Promise<ContratoRow> {
    this.validarUuid(contratoId, 'contrato');
    const fila = await this.pg.queryOne<ContratoRow>(
      `SELECT ${COLUMNAS}
         FROM orders.contratos
        WHERE id = $1 AND empresa_id = $2`,
      [contratoId, empresaId],
    );
    if (!fila) throw new NotFoundError('Contrato', contratoId);
    return fila;
  }

  private async registrarEvento(
    client: { query: (sql: string, params?: unknown[]) => Promise<unknown> },
    empresaId: string,
    contratoId: string,
    tipo: string,
    montoCents: number,
    motivo: string,
    actorId: string,
  ): Promise<void> {
    await client.query(
      `INSERT INTO orders.contrato_eventos
         (empresa_id, contrato_id, tipo, monto_cents, motivo, creado_por)
       VALUES ($1, $2, $3, $4, $5, $6)`,
      [empresaId, contratoId, tipo, montoCents, motivo, actorId],
    );
  }

  private serializar(fila: ContratoRow): Contrato {
    return {
      id: fila.id,
      empresa_id: fila.empresa_id,
      order_id: fila.order_id,
      parte_id: fila.parte_id,
      item_id: fila.item_id,
      tipo: fila.tipo,
      titulo: fila.titulo,
      descripcion: fila.descripcion,
      estado: fila.estado,
      inicia_en: fila.inicia_en,
      termina_en: fila.termina_en,
      deposito: Money.desdeCentavos(fila.deposito_cents).string(),
      garantia: Money.desdeCentavos(fila.garantia_cents).string(),
      retenido: Money.desdeCentavos(fila.retenido_cents).string(),
      moneda: fila.moneda,
      metadata: fila.metadata ?? {},
      creado_por: fila.creado_por,
      creado_en: fila.creado_en,
      actualizado_en: fila.actualizado_en,
      finalizado_en: fila.finalizado_en,
    };
  }

  private serializarEvento(fila: EventoRow): ContratoEvento {
    return {
      id: fila.id,
      contrato_id: fila.contrato_id,
      tipo: fila.tipo,
      monto: Money.desdeCentavos(fila.monto_cents).string(),
      motivo: fila.motivo,
      creado_por: fila.creado_por,
      creado_en: fila.creado_en,
    };
  }

  private validarTransicion(actual: EstadoContrato, nuevo: EstadoContrato): void {
    const permitidas: Record<EstadoContrato, EstadoContrato[]> = {
      borrador: ['borrador', 'activo', 'cancelado'],
      activo: ['activo', 'cancelado'],
      finalizado: [],
      cancelado: [],
    };
    if (!permitidas[actual].includes(nuevo)) {
      throw new ConflictError(`No se puede pasar de ${actual} a ${nuevo}.`);
    }
  }

  private validarTitulo(valor: unknown): string {
    const titulo = String(valor ?? '').trim();
    if (titulo.length < 2 || titulo.length > 200) {
      throw new DomainError('TITULO_INVALIDO', 'El titulo debe tener entre 2 y 200 caracteres.');
    }
    return titulo;
  }

  private validarTipo(valor: unknown): TipoContrato {
    const tipo = String(valor ?? 'otro') as TipoContrato;
    if (!TIPOS_CONTRATO.includes(tipo)) {
      throw new DomainError('TIPO_CONTRATO_INVALIDO', 'El tipo de contrato no es valido.');
    }
    return tipo;
  }

  private validarMoneda(valor: unknown): string {
    const moneda = String(valor ?? 'NIO').trim().toUpperCase();
    if (!MONEDA.test(moneda)) {
      throw new DomainError('MONEDA_INVALIDA', 'La moneda debe ser un codigo ISO de 3 letras.');
    }
    return moneda;
  }

  private validarMetadata(valor: unknown): Record<string, unknown> {
    if (valor === undefined || valor === null) return {};
    if (typeof valor !== 'object' || Array.isArray(valor)) {
      throw new DomainError('METADATA_INVALIDA', 'La metadata debe ser un objeto.');
    }
    return valor as Record<string, unknown>;
  }

  private validarFechas(
    iniciaValor: unknown,
    terminaValor: unknown,
  ): { inicia: string | null; termina: string | null } {
    const inicia = this.validarFecha(iniciaValor, 'inicia_en');
    const termina = this.validarFecha(terminaValor, 'termina_en');
    if (inicia && termina && termina < inicia) {
      throw new DomainError('FECHAS_INVALIDAS', 'La fecha final no puede ser anterior.');
    }
    return { inicia, termina };
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
