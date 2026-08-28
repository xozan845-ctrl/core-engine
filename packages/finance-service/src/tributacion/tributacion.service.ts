import { Injectable, OnModuleInit } from '@nestjs/common';
import * as cron from 'node-cron';
import { PgService, OutboxService, DomainError, ConflictError, EVENTOS, Logger } from '@core/shared';
import { ContabilidadService } from '../contabilidad/contabilidad.service';

export interface Jurisdiccion {
  codigo_pais: string;
  nombre: string;
  moneda: string;
  simbolo_moneda: string;
  tasa_iva: number;
  tasa_ir: number;
  periodicidad_declaracion: string;
  leyes: Record<string, string>;
  activa: boolean;
}

export interface RegimenFiscal {
  id: string;
  jurisdiccion: string;
  codigo: string;
  nombre: string;
  descripcion: string;
  periodicidad: string;
  condicion_ingresos_anuales_cents?: number;
  estado: 'activo' | 'inactivo';
}

export interface SujetoTributario {
  id: string;
  jurisdiccion: string;
  regimen_id?: string;
  regimen_nombre?: string;
  razon_social: string;
  ruc?: string;
  es_plataforma: boolean;
  estado: 'ACTIVO' | 'BAJA';
  creado_en: string;
}

export interface Declaracion {
  id: string;
  jurisdiccion: string;
  tipo: 'IVA' | 'IR' | 'CUOTA_FIJA';
  periodo_inicio: string;
  periodo_fin: string;
  base_gravada_cents: number;
  iva_debitado_cents: number;
  iva_credito_cents: number;
  iva_a_pagar_cents: number;
  renta_bruta_cents: number;
  renta_gravable_cents: number;
  ir_a_pagar_cents: number;
  cuota_cents: number;
  estado: 'PENDIENTE' | 'GENERADA' | 'PRESENTADA' | 'PAGADA' | 'CANCELADA';
  detalle?: unknown;
  generada_en?: string;
  presentada_en?: string;
  creado_en: string;
}

export interface PeriodoMensual {
  inicio: string;
  fin: string;
}

/**
 * Regimen fiscal (cap. 4.4) multijurisdiccion: cada pais es una fila en
 * finance.jurisdicciones (leyes, tasas y periodicidad configurables), por lo
 * que el mismo codigo sirve para adaptar el sistema a otros paises.
 * Nicaragua inicial: Ley 822 (IVA 15 %, IR) y regimenes DGI (Cuota Fija /
 * Regimen General). Las declaraciones son mensuales.
 */
@Injectable()
export class TributacionService implements OnModuleInit {
  private readonly logger = Logger.create('finance.tributacion');

  constructor(
    private readonly pg: PgService,
    private readonly outbox: OutboxService,
    private readonly contabilidad: ContabilidadService,
  ) {}

  /** Dias 1 a las 00:10: genera la declaracion del mes anterior (Ley 822). */
  onModuleInit(): void {
    cron.schedule('10 0 1 * *', () => {
      this.generarDeclaracionesDelMesAnterior(new Date()).catch((err) =>
        this.logger.error({ msg: 'Fallo la generacion mensual de declaraciones', err: err.message }),
      );
    });
    this.logger.info({ msg: 'Planificador mensual de declaraciones activo (dia 1)' });
  }

  // ── Jurisdicciones (adaptabilidad a otros paises) ───────────────────────

  async jurisdiccionActiva(): Promise<Jurisdiccion | null> {
    return this.pg.queryOne<Jurisdiccion>(
      `SELECT codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir,
              periodicidad_declaracion, leyes, activa
       FROM finance.jurisdicciones WHERE activa ORDER BY codigo_pais LIMIT 1`,
    );
  }

  async listarJurisdicciones(): Promise<Jurisdiccion[]> {
    const filas = await this.pg.query<Jurisdiccion>(
      `SELECT codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir,
              periodicidad_declaracion, leyes, activa
       FROM finance.jurisdicciones ORDER BY codigo_pais`,
    );
    return filas.map((f) => ({ ...f, tasa_iva: Number(f.tasa_iva), tasa_ir: Number(f.tasa_ir) }));
  }

  async crearJurisdiccion(datos: {
    codigo_pais: string;
    nombre: string;
    moneda: string;
    simbolo_moneda: string;
    tasa_iva: number;
    tasa_ir: number;
    periodicidad_declaracion?: string;
    leyes?: Record<string, string>;
  }): Promise<Jurisdiccion> {
    const fila = await this.pg.queryOne<Jurisdiccion>(
      `INSERT INTO finance.jurisdicciones
        (codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir,
         periodicidad_declaracion, leyes)
       VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, 'MENSUAL'), COALESCE($8::jsonb, '{}'))
       RETURNING codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir,
                 periodicidad_declaracion, leyes, activa`,
      [
        datos.codigo_pais.toUpperCase(),
        datos.nombre,
        datos.moneda,
        datos.simbolo_moneda,
        datos.tasa_iva,
        datos.tasa_ir,
        datos.periodicidad_declaracion ?? null,
        datos.leyes ? JSON.stringify(datos.leyes) : null,
      ],
    );
    if (!fila) throw new ConflictError('La jurisdiccion ya existe.');
    return { ...fila, tasa_iva: Number(fila.tasa_iva), tasa_ir: Number(fila.tasa_ir) };
  }

  // ── Regimenes fiscales ──────────────────────────────────────────────────

  async regimenesDe(jurisdiccion?: string): Promise<RegimenFiscal[]> {
    const filas = await this.pg.query<RegimenFiscal>(
      `SELECT id, jurisdiccion, codigo, nombre, descripcion, periodicidad,
              condicion_ingresos_anuales_cents, estado
       FROM finance.regimenes_fiscales
       WHERE ($1::text IS NULL OR jurisdiccion = $1)
       ORDER BY jurisdiccion, codigo`,
      [jurisdiccion ?? null],
    );
    return filas;
  }

  async crearRegimen(datos: {
    jurisdiccion: string;
    codigo: string;
    nombre: string;
    descripcion?: string;
    periodicidad?: string;
    condicion_ingresos_anuales_cents?: number;
  }): Promise<RegimenFiscal | null> {
    return this.pg.queryOne<RegimenFiscal>(
      `INSERT INTO finance.regimenes_fiscales
        (jurisdiccion, codigo, nombre, descripcion, periodicidad, condicion_ingresos_anuales_cents)
       VALUES ($1, $2, $3, COALESCE($4, ''), COALESCE($5, 'MENSUAL'), $6)
       RETURNING id, jurisdiccion, codigo, nombre, descripcion, periodicidad,
                 condicion_ingresos_anuales_cents, estado`,
      [
        datos.jurisdiccion.toUpperCase(),
        datos.codigo,
        datos.nombre,
        datos.descripcion ?? null,
        datos.periodicidad ?? null,
        datos.condicion_ingresos_anuales_cents ?? null,
      ],
    );
  }

  // ── Sujetos fiscales (vendedores y la plataforma) ───────────────────────

  async sujetos(): Promise<SujetoTributario[]> {
    return this.pg.query<SujetoTributario>(
      `SELECT s.id, s.jurisdiccion, s.regimen_id, r.nombre AS regimen_nombre,
              s.razon_social, s.ruc, s.es_plataforma, s.estado, s.creado_en
       FROM finance.sujetos_fiscales s
       LEFT JOIN finance.regimenes_fiscales r ON r.id = s.regimen_id
       ORDER BY s.creado_en DESC LIMIT 300`,
    );
  }

  async sujetoFiscalDe(usuarioId: string): Promise<SujetoTributario | null> {
    return this.pg.queryOne<SujetoTributario>(
      `SELECT s.id, s.jurisdiccion, s.regimen_id, r.nombre AS regimen_nombre,
              s.razon_social, s.ruc, s.es_plataforma, s.estado, s.creado_en
       FROM finance.sujetos_fiscales s
       LEFT JOIN finance.regimenes_fiscales r ON r.id = s.regimen_id
       WHERE s.id = $1`,
      [usuarioId],
    );
  }

  async registrarSujeto(datos: {
    id: string;
    jurisdiccion?: string;
    regimen_id?: string;
    razon_social: string;
    ruc?: string;
    es_plataforma?: boolean;
  }): Promise<SujetoTributario | null> {
    const jurisdiccion = datos.jurisdiccion ?? (await this.jurisdiccionActiva())?.codigo_pais;
    if (!jurisdiccion) throw new DomainError('SIN_JURISDICCION', 'No hay jurisdiccion activa configurada.');
    return this.pg.queryOne<SujetoTributario>(
      `INSERT INTO finance.sujetos_fiscales
        (id, jurisdiccion, regimen_id, razon_social, ruc, es_plataforma)
       VALUES ($1, $2, $3, $4, $5, COALESCE($6, FALSE))
       RETURNING id, jurisdiccion, regimen_id, razon_social, ruc, es_plataforma, estado, creado_en`,
      [datos.id, jurisdiccion, datos.regimen_id ?? null, datos.razon_social, datos.ruc ?? null, datos.es_plataforma ?? false],
    );
  }

  async darDeBaja(usuarioId: string): Promise<SujetoTributario | null> {
    return this.pg.queryOne<SujetoTributario>(
      `UPDATE finance.sujetos_fiscales SET estado = 'BAJA'
       WHERE id = $1 AND estado = 'ACTIVO'
       RETURNING id, jurisdiccion, regimen_id, razon_social, ruc, es_plataforma, estado, creado_en`,
      [usuarioId],
    );
  }

  // ── Declaraciones periodicas (mensuales, Ley 822) ───────────────────────

  /** Periodo mensual (inicio dia 1, fin ultimo dia) de una fecha. */
  periodoDelMes(fecha: Date = new Date()): PeriodoMensual {
    const anio = fecha.getFullYear();
    const mes = fecha.getMonth();
    return {
      inicio: new Date(anio, mes, 1).toISOString().slice(0, 10),
      fin: new Date(anio, mes + 1, 0).toISOString().slice(0, 10),
    };
  }

  /** Mes anterior al recibido: es el corte que genera la declaracion. */
  mesAnterior(fecha: Date = new Date()): PeriodoMensual {
    const previo = new Date(fecha.getFullYear(), fecha.getMonth(), 0);
    return this.periodoDelMes(previo);
  }

  async declaraciones(filtros?: {
    tipo?: string;
    periodo_inicio?: string;
    estado?: string;
  }): Promise<Declaracion[]> {
    return this.pg.query<Declaracion>(
      `SELECT id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
              base_gravada_cents, iva_debitado_cents, iva_credito_cents, iva_a_pagar_cents,
              renta_bruta_cents, renta_gravable_cents, ir_a_pagar_cents, cuota_cents,
              estado, detalle, generada_en, presentada_en, creado_en
       FROM finance.declaraciones
       WHERE ($1::text IS NULL OR tipo = $1)
         AND ($2::date IS NULL OR periodo_inicio = $2)
         AND ($3::text IS NULL OR estado = $3)
       ORDER BY periodo_inicio DESC, tipo LIMIT 300`,
      [filtros?.tipo ?? null, filtros?.periodo_inicio ?? null, filtros?.estado ?? null],
    );
  }

  async generarDeclaracionesDelMesAnterior(fecha = new Date()): Promise<Declaracion[]> {
    return this.generarDeclaraciones(this.mesAnterior(fecha));
  }

  /**
   * Genera las declaraciones IVA e IR (mas CUOTA_FIJA informativa) del
   * periodo usando la base contable real (partida doble) y la tasa de la
   * jurisdiccion activa. Idempotente por (jurisdiccion, tipo, periodo).
   */
  async generarDeclaraciones(periodo?: PeriodoMensual): Promise<Declaracion[]> {
    const jurisdiccion = await this.jurisdiccionActiva();
    if (!jurisdiccion) {
      throw new DomainError('SIN_JURISDICCION', 'No hay jurisdiccion activa configurada.');
    }
    const mes = periodo ?? this.mesAnterior();
    const ingresos = await this.contabilidad.ingresosDelPeriodo(mes.inicio, mes.fin);
    const gastos = await this.contabilidad.gastosDelPeriodo(mes.inicio, mes.fin);

    const iiva = Math.round(ingresos * Number(jurisdiccion.tasa_iva));
    const irBase = Math.max(0, ingresos - gastos);
    const iir = Math.round(irBase * Number(jurisdiccion.tasa_ir));

    const plan: {
      tipo: Declaracion['tipo'];
      campos: Record<string, number>;
      detalle: Record<string, unknown>;
    }[] = [
      {
        tipo: 'IVA',
        campos: {
          base_gravada_cents: ingresos,
          iva_debitado_cents: iiva,
          iva_credito_cents: 0,
          iva_a_pagar_cents: iiva,
        },
        detalle: { tasa: Number(jurisdiccion.tasa_iva), ley: jurisdiccion.leyes?.iva ?? 'Ley 822' },
      },
      {
        tipo: 'IR',
        campos: {
          renta_bruta_cents: ingresos,
          renta_gravable_cents: irBase,
          ir_a_pagar_cents: iir,
        },
        detalle: {
          tasa: Number(jurisdiccion.tasa_ir),
          gastos_del_periodo_cents: gastos,
          ley: 'Ley 822 (IR)',
        },
      },
      {
        tipo: 'CUOTA_FIJA',
        campos: { cuota_cents: 0 },
        detalle: {
          informativo: true,
          nota: 'Cuota determinada por la DGI para cada contribuyente; la plataforma documenta comisiones para su declaracion.',
        },
      },
    ];

    const generadas: Declaracion[] = [];
    for (const item of plan) {
      if (item.tipo !== 'IVA' && item.tipo !== 'IR' && ingresos === 0 && gastos === 0) {
        continue; // sin actividad: no se declara
      }
      const existente = await this.pg.queryOne<Declaracion>(
        `SELECT id FROM finance.declaraciones WHERE jurisdiccion = $1 AND tipo = $2 AND periodo_inicio = $3`,
        [jurisdiccion.codigo_pais, item.tipo, mes.inicio],
      );
      if (existente) {
        generadas.push(await this.declaracion(existente.id));
        continue;
      }

      const declaracion: Declaracion = await this.pg.transaccion(async (client) => {
        const fila = await client.query<Declaracion>(
          `INSERT INTO finance.declaraciones
            (jurisdiccion, tipo, periodo_inicio, periodo_fin,
             base_gravada_cents, iva_debitado_cents, iva_credito_cents, iva_a_pagar_cents,
             renta_bruta_cents, renta_gravable_cents, ir_a_pagar_cents, cuota_cents,
             estado, detalle, generada_en)
           VALUES ($1, $2, $3, $4,
                   COALESCE($5, 0), COALESCE($6, 0), COALESCE($7, 0), COALESCE($8, 0),
                   COALESCE($9, 0), COALESCE($10, 0), COALESCE($11, 0), COALESCE($12, 0),
                   'GENERADA', $13, NOW())
           RETURNING id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                     base_gravada_cents, iva_debitado_cents, iva_credito_cents, iva_a_pagar_cents,
                     renta_bruta_cents, renta_gravable_cents, ir_a_pagar_cents, cuota_cents,
                     estado, detalle, generada_en, presentada_en, creado_en`,
          [
            jurisdiccion.codigo_pais,
            item.tipo,
            mes.inicio,
            mes.fin,
            (item.campos.base_gravada_cents as number) ?? 0,
            (item.campos.iva_debitado_cents as number) ?? 0,
            (item.campos.iva_credito_cents as number) ?? 0,
            (item.campos.iva_a_pagar_cents as number) ?? 0,
            (item.campos.renta_bruta_cents as number) ?? 0,
            (item.campos.renta_gravable_cents as number) ?? 0,
            (item.campos.ir_a_pagar_cents as number) ?? 0,
            (item.campos.cuota_cents as number) ?? 0,
            JSON.stringify(item.detalle),
          ],
        );
        const creada = fila.rows[0];
        await this.outbox.insertarEnTransaccion(client, EVENTOS.DECLARACION_GENERADA, {
          declaracion_id: creada.id,
          jurisdiccion: creada.jurisdiccion,
          tipo: creada.tipo,
          periodo_inicio: mes.inicio,
          periodo_fin: mes.fin,
          a_pagar_cents:
            creada.tipo === 'IVA'
              ? creada.iva_a_pagar_cents
              : creada.tipo === 'IR'
                ? creada.ir_a_pagar_cents
                : creada.cuota_cents,
          estado: creada.estado,
        });
        return creada;
      });
      generadas.push(declaracion);
      this.logger.info({
        msg: 'Declaracion generada',
        tipo: declaracion.tipo,
        periodo: `${mes.inicio}..${mes.fin}`,
      });
    }
    return generadas;
  }

  private async declaracion(id: string): Promise<Declaracion> {
    const fila = await this.pg.queryOne<Declaracion>(
      `SELECT id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
              base_gravada_cents, iva_debitado_cents, iva_credito_cents, iva_a_pagar_cents,
              renta_bruta_cents, renta_gravable_cents, ir_a_pagar_cents, cuota_cents,
              estado, detalle, generada_en, presentada_en, creado_en
       FROM finance.declaraciones WHERE id = $1`,
      [id],
    );
    if (!fila) throw new DomainError('DECLARACION_INEXISTENTE', `Declaracion ${id} no existe.`);
    return fila;
  }

  async presentar(id: string): Promise<Declaracion | null> {
    const fila = await this.pg.queryOne<Declaracion>(
      `UPDATE finance.declaraciones SET estado = 'PRESENTADA', presentada_en = NOW()
       WHERE id = $1 AND estado IN ('GENERADA', 'PENDIENTE')
       RETURNING id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                 base_gravada_cents, iva_debitado_cents, iva_credito_cents, iva_a_pagar_cents,
                 renta_bruta_cents, renta_gravable_cents, ir_a_pagar_cents, cuota_cents,
                 estado, detalle, generada_en, presentada_en, creado_en`,
      [id],
    );
    return fila;
  }

  async marcarPagada(id: string): Promise<Declaracion | null> {
    return this.pg.queryOne<Declaracion>(
      `UPDATE finance.declaraciones SET estado = 'PAGADA'
       WHERE id = $1 AND estado IN ('GENERADA', 'PRESENTADA')
       RETURNING id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                 base_gravada_cents, iva_debitado_cents, iva_credito_cents, iva_a_pagar_cents,
                 renta_bruta_cents, renta_gravable_cents, ir_a_pagar_cents, cuota_cents,
                 estado, detalle, generada_en, presentada_en, creado_en`,
      [id],
    );
  }
}