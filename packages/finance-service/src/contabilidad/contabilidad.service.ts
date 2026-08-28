import { Injectable } from '@nestjs/common';
import { PgService, OutboxService, Money, DomainError, EVENTOS, Logger } from '@core/shared';
import type { PoolClient } from 'pg';

export interface DetalleAsiento {
  cuenta_codigo: string;
  debe_cents: number;
  haber_cents: number;
  concepto?: string;
  orden?: number;
}

export interface ParametrosAsiento {
  concepto: string;
  tipo: 'INGRESO' | 'EGRESO' | 'AJUSTE' | 'CIERRE' | 'APERTURA' | 'MANUAL';
  referencia_tipo?: string;
  referencia_id?: string;
  fecha?: string;
  moneda?: string;
  creado_por?: string;
  detalles: DetalleAsiento[];
}

export interface Asiento {
  id: string;
  fecha: string;
  concepto: string;
  tipo: string;
  referencia_tipo?: string;
  referencia_id?: string;
  moneda: string;
  estado: string;
  creado_por?: string;
  creado_en: string;
  debe_cents: number;
  haber_cents: number;
  detalles: DetalleAsiento[];
}

export interface Cuenta {
  codigo: string;
  nombre: string;
  tipo: 'ACTIVO' | 'PASIVO' | 'CAPITAL' | 'INGRESO' | 'COSTO' | 'GASTO';
  naturaleza: 'DEUDORA' | 'ACREEDORA';
  nivel: number;
  estado: 'activa' | 'inactiva';
}

/**
 * Contabilidad por partida doble (cap. 4.4: libros y registros contables
 * conformes a la DGI). La bitacora es append-only: un asiento nunca se edita
 * ni se borra, se anula (auditoria). Cada asiento se publica al bus como
 * `asiento.registrado` para trazabilidad.
 */
@Injectable()
export class ContabilidadService {
  private readonly logger = Logger.create('finance.contabilidad');

  constructor(
    private readonly pg: PgService,
    private readonly outbox: OutboxService,
  ) {}

  /** Partida doble: debe == haber y al menos un movimiento por lado. */
  validarPartidaDoble(detalles: DetalleAsiento[]): void {
    if (!detalles || detalles.length < 2) {
      throw new DomainError('PARTIDA_DOBLE_INVALIDA', 'Un asiento requiere al menos dos movimientos.');
    }
    const debe = detalles.reduce((s, d) => s + d.debe_cents, 0);
    const haber = detalles.reduce((s, d) => s + d.haber_cents, 0);
    if (debe <= 0 || haber <= 0 || debe !== haber) {
      throw new DomainError(
        'PARTIDA_DOBLE_DESCUADRADA',
        `El asiento no cuadra: debe ${debe} no es igual a haber ${haber}.`,
      );
    }
  }

  /** Inserta asiento + detalles DENTRO de una transaccion (ADR-03). */
  async registrarEnTransaccion(client: PoolClient, params: ParametrosAsiento): Promise<Asiento> {
    this.validarPartidaDoble(params.detalles);
    const fila = await client.query<Asiento>(
      `INSERT INTO finance.asientos (fecha, concepto, tipo, referencia_tipo, referencia_id, moneda, creado_por)
       VALUES (COALESCE($1::timestamptz, NOW()), $2, $3, $4, $5, COALESCE($6, 'C$'), $7)
       RETURNING id, fecha, concepto, tipo, referencia_tipo, referencia_id, moneda, estado, creado_por, creado_en`,
      [
        params.fecha ?? null,
        params.concepto,
        params.tipo,
        params.referencia_tipo ?? null,
        params.referencia_id ?? null,
        params.moneda ?? 'C$',
        params.creado_por ?? null,
      ],
    );
    const asiento = fila.rows[0];
    for (const [indice, d] of params.detalles.entries()) {
      await client.query(
        `INSERT INTO finance.asiento_detalles (asiento_id, cuenta_codigo, debe_cents, haber_cents, concepto, orden)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [asiento.id, d.cuenta_codigo, d.debe_cents, d.haber_cents, d.concepto ?? null, d.orden ?? indice],
      );
    }
    const debe = params.detalles.reduce((s, d) => s + d.debe_cents, 0);
    const haber = params.detalles.reduce((s, d) => s + d.haber_cents, 0);
    await this.outbox.insertarEnTransaccion(client, EVENTOS.ASIENTO_REGISTRADO, {
      asiento_id: asiento.id,
      fecha: asiento.fecha,
      tipo: asiento.tipo,
      concepto: asiento.concepto,
      referencia_tipo: asiento.referencia_tipo,
      referencia_id: asiento.referencia_id,
      debe_cents: debe,
      haber_cents: haber,
      moneda: asiento.moneda,
    });
    return { ...asiento, debe_cents: debe, haber_cents: haber, detalles: params.detalles };
  }

  /** Registra un asiento en su propia transaccion con publicacion via outbox. */
  async registrar(params: ParametrosAsiento): Promise<Asiento> {
    return this.pg.transaccion(async (client) => this.registrarEnTransaccion(client, params));
  }

  /** Anula un asiento: solo cambia su estado (append-only, auditoria DGI). */
  async anular(id: string): Promise<Asiento | null> {
    const fila = await this.pg.queryOne<Asiento>(
      `UPDATE finance.asientos SET estado = 'ANULADO'
       WHERE id = $1 AND estado = 'REGISTRADO'
       RETURNING id, fecha, concepto, tipo, referencia_tipo, referencia_id, moneda, estado, creado_por, creado_en`,
      [id],
    );
    if (!fila) {
      const existente = await this.pg.queryOne(`SELECT id FROM finance.asientos WHERE id = $1`, [id]);
      if (!existente) return null;
    }
    return fila;
  }

  /** Libro diario: bitacora cronologica con totales y detalle (DGI).
   * CQRS (ADR-07): lee la proyeccion materializada finance.asientos_vista, que
   * evita JOINs sobre las tablas de escritura. La vista preserva el shape
   * append-only incluyendo asientos anulados (estado). */
  async libroDiario(desde?: string, hasta?: string, limite = 200): Promise<Asiento[]> {
    const filas = await this.pg.query<Asiento>(
      `SELECT id, fecha, concepto, tipo, referencia_tipo, referencia_id, moneda, estado,
              creado_por, creado_en, debe_cents, haber_cents, detalles_jsonb AS detalles
       FROM finance.asientos_vista
       WHERE ($1::timestamptz IS NULL OR fecha >= $1)
         AND ($2::timestamptz IS NULL OR fecha <= $2)
       ORDER BY fecha DESC, id
       LIMIT $3`,
      [desde ?? null, hasta ?? null, Math.min(500, Math.max(1, limite))],
    );
    return filas;
  }

  /** Libro mayor: saldo inicial + movimientos + saldo final por cuenta.
   * CQRS (ADR-07): lee finance.libro_mayor_mv (proyeccion materializada) en vez
   * de JOINs crudos sobre asientos+detalles. Saldo inicial se calcula con la
   * acumulacion de periodos previos (append-only). */
  async libroMayor(
    cuentaCodigo: string,
    desde?: string,
    hasta?: string,
  ): Promise<{ cuenta: Cuenta | null; saldo_inicial_cents: number; saldo_final_cents: number; movimientos: unknown[] }> {
    const cuenta = await this.pg.queryOne<Cuenta>(
      `SELECT codigo, nombre, tipo, naturaleza, nivel, estado FROM finance.plan_cuentas WHERE codigo = $1`,
      [cuentaCodigo],
    );
    if (!cuenta) throw new DomainError('CUENTA_INEXISTENTE', `La cuenta ${cuentaCodigo} no existe en el plan.`);

    const fin = hasta ?? new Date().toISOString();
    const inicioMes = `${fin.slice(0, 7)}-01`;
    // saldo inicial: acumulacion de movimientos de periodos previos a 'inicioMes'
    const previo = await this.pg.queryOne<{ saldo: number }>(
      `SELECT COALESCE(SUM(debe_cents) - SUM(haber_cents), 0)::int AS saldo
       FROM finance.libro_mayor_mv
       WHERE cuenta_codigo = $1 AND periodo < $2::date`,
      [cuentaCodigo, inicioMes],
    );
    const saldoEn = (debe: number, haber: number): number =>
      cuenta.naturaleza === 'DEUDORA' ? debe - haber : haber - debe;
    const saldoInicial = saldoEn(previo?.saldo ?? 0, 0);

    const movimientos = await this.pg.query<unknown>(
      `SELECT m AS movimiento FROM finance.libro_mayor_mv,
              jsonb_array_elements(movimientos_jsonb) AS m
       WHERE cuenta_codigo = $1 AND periodo = $2::date
       ORDER BY (m->>'fecha')::timestamptz, m->>'asiento_id'`,
      [cuentaCodigo, inicioMes],
    );
    const flujo = (movimientos as Array<{ movimiento: { debe_cents: number; haber_cents: number } }>).reduce(
      (s, m) => s + saldoEn(Number(m.movimiento.debe_cents), Number(m.movimiento.haber_cents)),
      0,
    );
    return {
      cuenta,
      saldo_inicial_cents: saldoInicial,
      saldo_final_cents: saldoInicial + flujo,
      movimientos,
    };
  }

  /** Suma de ingresos del periodo (declaraciones de IR/IVA, cap. 4.4).
   * CQRS: agrega sobre finance.libro_mayor_mv + plan_cuentas (sin JOIN a detalles). */
  async ingresosDelPeriodo(desde: string, hasta: string): Promise<number> {
    const fila = await this.pg.queryOne<{ total: number }>(
      `SELECT COALESCE(SUM(lm.debe_cents - lm.haber_cents), 0)::int AS total
       FROM finance.libro_mayor_mv lm
       JOIN finance.plan_cuentas c ON c.codigo = lm.cuenta_codigo
       WHERE c.tipo = 'INGRESO'
         AND lm.periodo >= date_trunc('month', $1::timestamptz)::date
         AND lm.periodo <= date_trunc('month', $2::timestamptz)::date`,
      [desde, hasta],
    );
    return fila?.total ?? 0;
  }

  /** Suma de gastos y costos del periodo (base de la renta gravable).
   * CQRS: agrega sobre finance.libro_mayor_mv + plan_cuentas (sin JOIN a detalles). */
  async gastosDelPeriodo(desde: string, hasta: string): Promise<number> {
    const fila = await this.pg.queryOne<{ total: number }>(
      `SELECT COALESCE(SUM(lm.haber_cents - lm.debe_cents), 0)::int AS total
       FROM finance.libro_mayor_mv lm
       JOIN finance.plan_cuentas c ON c.codigo = lm.cuenta_codigo
       WHERE c.tipo IN ('GASTO', 'COSTO')
         AND lm.periodo >= date_trunc('month', $1::timestamptz)::date
         AND lm.periodo <= date_trunc('month', $2::timestamptz)::date`,
      [desde, hasta],
    );
    return fila?.total ?? 0;
  }

  /** Plan de cuentas (libros contables conformes a la DGI). */
  async planDeCuentas(): Promise<Cuenta[]> {
    return this.pg.query<Cuenta>(
      `SELECT codigo, nombre, tipo, naturaleza, nivel, estado FROM finance.plan_cuentas ORDER BY codigo`,
    );
  }

  async crearCuenta(datos: {
    codigo: string;
    nombre: string;
    tipo: Cuenta['tipo'];
    naturaleza: Cuenta['naturaleza'];
    nivel?: number;
  }): Promise<Cuenta> {
    if (!/^\d+(\.\d+)*$/.test(datos.codigo)) {
      throw new DomainError('CODIGO_CUENTA_INVALIDO', 'El codigo de cuenta debe ser jerarquico (ej. 1.1.3).');
    }
    const fila = await this.pg.queryOne<Cuenta>(
      `INSERT INTO finance.plan_cuentas (codigo, nombre, tipo, naturaleza, nivel)
       VALUES ($1, $2, $3, $4, $5)
       RETURNING codigo, nombre, tipo, naturaleza, nivel, estado`,
      [datos.codigo, datos.nombre, datos.tipo, datos.naturaleza, datos.nivel ?? 3],
    );
    if (!fila) throw new DomainError('CUENTA_DUPLICADA', `La cuenta ${datos.codigo} ya existe.`);
    return fila;
  }

  async estadoCuenta(codigo: string, estado: 'activa' | 'inactiva'): Promise<Cuenta | null> {
    return this.pg.queryOne<Cuenta>(
      `UPDATE finance.plan_cuentas SET estado = $2
       WHERE codigo = $1 RETURNING codigo, nombre, tipo, naturaleza, nivel, estado`,
      [codigo, estado],
    );
  }

  /**
   * Libro de ventas (DGI): consolidado mensual de los comprobantes EMITIDOS
   * con base gravada, IVA (15 % Ley 822), exento y total. Complementa el
   * libro diario/mayor exigido en el cap. 4.4.
   */
  async libroVentas(desde?: string, hasta?: string): Promise<LibroIva[]> {
    return this.pg.query<LibroIva>(
      `SELECT TO_CHAR(emitido_en, 'YYYY-MM') AS periodo,
              COUNT(*)::int AS comprobantes,
              COALESCE(SUM(base_gravada_cents), 0)::int AS base_gravada_cents,
              COALESCE(SUM(iva_cents), 0)::int AS iva_cents,
              COALESCE(SUM(exento_cents), 0)::int AS exento_cents,
              COALESCE(SUM(total_cents), 0)::int AS total_cents
       FROM finance.comprobantes
       WHERE estado = 'EMITIDO'
         AND ($1::timestamptz IS NULL OR emitido_en >= $1)
         AND ($2::timestamptz IS NULL OR emitido_en <= $2)
       GROUP BY TO_CHAR(emitido_en, 'YYYY-MM')
       ORDER BY periodo`,
      [desde ?? null, hasta ?? null],
    );
  }

  /**
   * Libro de compras (DGI): en el MVP no se registran compras de bienes y
   * servicios a terceros, por lo que el credito fiscal (IVA por acreditar)
   * es cero; la estructura queda lista para cuando el sistema registre
   * compras (credito fiscal de la declaracion mensual de IVA, Ley 822).
   */
  async libroCompras(desde?: string, hasta?: string): Promise<LibroIva[]> {
    return this.pg.query<LibroIva>(
      `SELECT TO_CHAR(creado_en, 'YYYY-MM') AS periodo,
              0::int AS comprobantes,
              0::int AS base_gravada_cents,
              0::int AS iva_cents,
              0::int AS exento_cents,
              0::int AS total_cents
       FROM finance.comprobantes
       WHERE ($1::timestamptz IS NULL OR creado_en >= $1)
         AND ($2::timestamptz IS NULL OR creado_en <= $2)
       GROUP BY TO_CHAR(creado_en, 'YYYY-MM')
       ORDER BY periodo`,
      [desde ?? null, hasta ?? null],
    );
  }

  /** Formato moneda para respuestas (doc: montos en C$ con dos decimales). */
  formatear(centavos: number): string {
    return Money.desdeCentavos(centavos).string();
  }
}

export interface LibroIva {
  periodo: string;
  comprobantes: number;
  base_gravada_cents: number;
  iva_cents: number;
  exento_cents: number;
  total_cents: number;
}