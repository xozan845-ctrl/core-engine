import { Injectable } from '@nestjs/common';
import { PgService, Money, DomainError, Logger } from '@core/shared';

export interface SupuestosProyeccion {
  horizonte_meses: number;
  /** Rampa de vendedores activos (tabla 8.6: 20 al mes 1, +5 netos al mes). */
  vendedores_iniciales: number;
  entrada_vendedores_mes: number;
  churn_tasa: number; // por mes, 0..1 (tabla 8.6: < 5 %)
  /** Rampa de pedidos por vendedor (tabla 8.6: 25..42, "+1 a +2 al mes"). */
  pedidos_por_vendedor: number[];
  ticket_promedio_cents: number; // tabla 8.6: C$ 450
  comision_tasa: number; // RN-04: 0.12
  costos_fijos_cents: number; // C$ 2 000 (meses 1-6)
  costos_fijos_desde_mes7_cents?: number; // C$ 3 000 (meses 7-12)
  inversion_inicial_cents: number; // tabla 8.11: C$ 58 500
  tasa_descuento_mensual?: number; // 1,5 % (18 % anual / 12)
}

export interface FilaProyeccion {
  mes: number;
  vendedores: number;
  pedidos: number;
  gmv_cents: number;
  ingresos_cents: number;
  costos_cents: number;
  flujo_cents: number;
  flujo_acumulado_cents: number;
  /** Economia de escala (8.7): costo fijo de plataforma por pedido (C$ 4,00 m1 -> C$ 0,95 m12). */
  costo_fijo_por_pedido_cents: number;
}

export interface TotalesProyeccion {
  gmv_cents: number;
  ingresos_cents: number;
  costos_cents: number;
  flujo_cents: number;
}

export interface EscenarioEquilibrio {
  escenario: 'bajo' | 'medio' | 'alto';
  costo_variable_por_pedido_cents: number;
  contribucion_por_pedido_cents: number;
  pedidos_equilibrio: number;
  gmv_equilibrio_cents: number;
}

export interface IndicadoresProyeccion {
  payback_mes: number | null;
  van_cents: number;
  tir_mensual: number | null;
  tir_anual: number | null;
  tir_no_estable: boolean; // doc 8.11: "TIR > 100 % (no estable)"
  roi: number;
  margen_neto: number;
  punto_equilibrio_caja_cents: number;
  pedidos_equilibrio_caja: number;
  escenarios_equilibrio: EscenarioEquilibrio[];
  margen_seguridad_por_mes: { mes: number; gmv_cents: number; margen_seguridad: number }[];
  /** Cobertura por vendedor (8.7): C$ 54 x 42 = C$ 2 268; ~4 vendedores cubren C$ 3 000. */
  cobertura_por_vendedor: CoberturaPorVendedor;
  /** Costo fijo de plataforma por pedido y multiplicador de escala (8.7). */
  economia_de_escala: EconomiaDeEscala;
}

export interface CoberturaPorVendedor {
  pedidos_por_vendedor_mes: number;
  comision_por_pedido_cents: number; // C$ 54
  comision_bruta_vendedor_cents: number; // C$ 54 x 42 = C$ 2 268
  contribucion_media_cents: number; // C$ 18 (escenario medio)
  vendedores_para_cubrir_costos: number; // ceil(3 000 / (18 x 42)) = 4
}

export interface EconomiaDeEscala {
  costo_fijo_por_pedido_inicio_cents: number; // C$ 4,00 (mes 1)
  costo_fijo_por_pedido_mes6_cents: number; // C$ 1,27
  costo_fijo_por_pedido_fin_cents: number; // C$ 0,95 (mes 12)
  multiplicador_escala: number; // 4,0 / 0,95 ~= 4,2x
}

/** Fila de la matriz de sensibilidad comision x GMV (tabla 8.10). */
export interface SensibilidadComisionGmv {
  tasa_comision: number;
  flujo_mes12_cents: { variacion_gmv: number; flujo_cents: number }[];
}

/** Sensibilidad del VAN al costo de oportunidad (tabla 8.12). */
export interface SensibilidadVan {
  tasa_anual: number;
  tasa_mensual: number;
  van_cents: number;
  relacion_van_inversion: number;
}

/** Payback por escenario (tabla 8.12). */
export interface PaybackPorEscenario {
  escenario: 'base' | 'pesimista' | 'peor_combinado';
  descripcion: string;
  flujo_acumulado_cents: number;
  payback_mes: number | null;
}

/** Cuenta de resultados resumida por anio (tabla 8.11). */
export interface ResumenAnual {
  anio: number;
  gmv_cents: number;
  ingresos_cents: number;
  costos_cents: number;
  flujo_cents: number;
  margen_neto: number;
}

export interface PlanBienal {
  inversion_inicial_cents: number;
  anios: ResumenAnual[];
  filas: FilaProyeccion[];
  acumulado_24_meses: {
    gmv_cents: number;
    ingresos_cents: number;
    costos_cents: number;
    flujo_cents: number;
  };
  roi: number; // 3 669 396 / 58 500 ~= 63x
  mes_24: { gmv_cents: number; incremento_vs_mes12: number };
  indicadores: IndicadoresProyeccion;
}

export interface Proyeccion {
  id: string;
  nombre: string;
  horizonte_meses: number;
  supuestos: SupuestosProyeccion;
  filas: FilaProyeccion[];
  totales: TotalesProyeccion;
  inversion_inicial_cents: number;
  indicadores: IndicadoresProyeccion;
  creado_en: string;
}

/**
 * Proyeccion financiera del cap. 8: modelo de calculo mes a mes (tabla 8.7)
 * con formulas cerradas y auditables; punto de equilibrio (8.7), indicadores
 * de rentabilidad VAN/TIR/payback (8.11) y margen de seguridad (8.7).
 * Todas las cifras en centavos; compatibles con la tabla 8.9 del informe.
 */
@Injectable()
export class ProyeccionesService {
  private readonly logger = Logger.create('finance.proyecciones');

  constructor(private readonly pg: PgService) {}

  /** Modelo cerrado: vendedores -> pedidos -> GMV -> ingresos -> flujo. */
  calcular(supuestos: SupuestosProyeccion): {
    filas: FilaProyeccion[];
    totales: TotalesProyeccion;
  } {
    const n = supuestos.horizonte_meses;
    if (supuestos.pedidos_por_vendedor.length !== n) {
      throw new DomainError(
        'RAMPRA_INCOMPLETA',
        `La rampa de pedidos por vendedor debe tener ${n} valores (tabla 8.6).`,
      );
    }

    const filas: FilaProyeccion[] = [];
    let vendedores = supuestos.vendedores_iniciales;
    let acumulado = 0;
    const costosEtapa2 = supuestos.costos_fijos_desde_mes7_cents ?? supuestos.costos_fijos_cents;

    for (let t = 1; t <= n; t += 1) {
      if (t > 1) {
        const churn = Math.round(vendedores * supuestos.churn_tasa);
        vendedores = vendedores - churn + supuestos.entrada_vendedores_mes;
      }
      const pedidos = vendedores * supuestos.pedidos_por_vendedor[t - 1];
      const gmv = pedidos * supuestos.ticket_promedio_cents;
      const ingresos = Math.round(gmv * supuestos.comision_tasa);
      const costos = t <= 6 ? supuestos.costos_fijos_cents : costosEtapa2;
      const flujo = ingresos - costos;

      const fila: FilaProyeccion = {
        mes: t,
        vendedores,
        pedidos,
        gmv_cents: gmv,
        ingresos_cents: ingresos,
        costos_cents: costos,
        flujo_cents: flujo,
        flujo_acumulado_cents: acumulado + flujo,
        costo_fijo_por_pedido_cents: pedidos > 0 ? Math.round(costos / pedidos) : 0,
      };
      acumulado = fila.flujo_acumulado_cents;
      filas.push(fila);
    }

    return {
      filas,
      totales: {
        gmv_cents: filas.reduce((s, f) => s + f.gmv_cents, 0),
        ingresos_cents: filas.reduce((s, f) => s + f.ingresos_cents, 0),
        costos_cents: filas.reduce((s, f) => s + f.costos_cents, 0),
        flujo_cents: filas.reduce((s, f) => s + f.flujo_cents, 0),
      },
    };
  }

  /** Indicadores de rentabilidad (8.11) y equilibrio (8.7) sobre las filas. */
  indicadores(filas: FilaProyeccion[], supuestos: SupuestosProyeccion): IndicadoresProyeccion {
    const inversion = supuestos.inversion_inicial_cents;
    const tasaMensual = supuestos.tasa_descuento_mensual ?? 0.015;
    const ticket = supuestos.ticket_promedio_cents;
    const tasa = supuestos.comision_tasa;
    const costosEtapa2 = supuestos.costos_fijos_desde_mes7_cents ?? supuestos.costos_fijos_cents;
    const costosFijos = filas.length >= 7 ? costosEtapa2 : supuestos.costos_fijos_cents;

    // payback: primer mes con flujo acumulado >= inversion (tabla 8.11)
    const payback = filas.find((f) => f.flujo_acumulado_cents >= inversion)?.mes ?? null;

    // VAN: flujos descontados a la tasa de costo de oportunidad (18 % anual / 12)
    const van = filas.reduce((s, f, i) => s + f.flujo_cents / Math.pow(1 + tasaMensual, i + 1), 0) - inversion;

    // TIR: tasa mensual que anula el VAN (bisqueda binaria; ver nota 8.11)
    const tir = this.tirMensual(filas, inversion);
    const tirAnual = tir === null ? null : (Math.pow(1 + tir, 12) - 1) * 100;

    // punto de equilibrio de caja: costos / comision (8.7: C$ 25 000 de GMV)
    const equilibrioCaja = costosFijos / tasa;
    const pedidosEquilibrio = Math.max(1, Math.ceil(equilibrioCaja / ticket));

    // punto de equilibrio con costos variables (tabla 8.12 / 8.7)
    const ingresoPorPedido = Money.desdeCentavos(ticket).multiplicarPor(tasa).centavos;
    const escenarios: { escenario: EscenarioEquilibrio['escenario']; cv: number }[] = [
      { escenario: 'bajo', cv: 2000 }, // C$ 20: last-mile eficiente
      { escenario: 'medio', cv: 3600 }, // C$ 36: promedio del rango
      { escenario: 'alto', cv: 5200 }, // C$ 52: last-mile costoso
    ];
    const escenariosEquilibrio: EscenarioEquilibrio[] = escenarios.map(({ escenario, cv }) => {
      const contribucion = ingresoPorPedido - cv;
      const pedidos = Math.ceil(costosFijos / contribucion);
      return {
        escenario,
        costo_variable_por_pedido_cents: cv,
        contribucion_por_pedido_cents: contribucion,
        pedidos_equilibrio: pedidos,
        gmv_equilibrio_cents: pedidos * ticket,
      };
    });

    // margen de seguridad mensual frente al equilibrio medio (8.7, tabla 8.13)
    const equilibrioMedio = escenariosEquilibrio.find((e) => e.escenario === 'medio')?.gmv_equilibrio_cents ?? 0;
    const margenSeguridad = filas.filter((f) => [1, 3, 6, 12].includes(f.mes)).map((f) => ({
      mes: f.mes,
      gmv_cents: f.gmv_cents,
      margen_seguridad: equilibrioMedio > 0 ? 1 - equilibrioMedio / f.gmv_cents : 0,
    }));

    const totalFlujo = filas.reduce((s, f) => s + f.flujo_cents, 0);
    const totalIngresos = filas.reduce((s, f) => s + f.ingresos_cents, 0);

    return {
      payback_mes: payback,
      van_cents: Math.round(van),
      tir_mensual: tir,
      tir_anual: tirAnual === null ? null : Math.round(tirAnual * 100) / 100,
      tir_no_estable: tirAnual === null || tirAnual > 100,
      roi: inversion > 0 ? totalFlujo / inversion : 0,
      margen_neto: totalIngresos > 0 ? totalFlujo / totalIngresos : 0,
      punto_equilibrio_caja_cents: Math.round(equilibrioCaja),
      pedidos_equilibrio_caja: pedidosEquilibrio,
      escenarios_equilibrio: escenariosEquilibrio,
      margen_seguridad_por_mes: margenSeguridad,
      cobertura_por_vendedor: this.coberturaPorVendedor(supuestos),
      economia_de_escala: this.economiaDeEscala(filas),
    };
  }

  /**
   * Cobertura por vendedor (8.7): un vendedor con 42 pedidos/mes genera
   * C$ 54 x 42 = C$ 2 268 de comision bruta; con contribucion media
   * (C$ 18/pedido) bastan ~4 vendedores activos para cubrir los costos fijos.
   */
  coberturaPorVendedor(supuestos: SupuestosProyeccion): CoberturaPorVendedor {
    const pedidosPorVendedor = supuestos.pedidos_por_vendedor[supuestos.pedidos_por_vendedor.length - 1] ?? 25;
    const comisionPorPedido = Math.round(
      Money.desdeCentavos(supuestos.ticket_promedio_cents).multiplicarPor(supuestos.comision_tasa).centavos,
    );
    const contribucionMedia = 1800; // C$ 18/pedido (escenario medio, tabla 8.12)
    const costosFijos =
      (supuestos.pedidos_por_vendedor.length >= 7 ? supuestos.costos_fijos_desde_mes7_cents : null) ??
      supuestos.costos_fijos_cents;
    const vendedores = Math.ceil(costosFijos / (contribucionMedia * pedidosPorVendedor));
    return {
      pedidos_por_vendedor_mes: pedidosPorVendedor,
      comision_por_pedido_cents: comisionPorPedido,
      comision_bruta_vendedor_cents: comisionPorPedido * pedidosPorVendedor,
      contribucion_media_cents: contribucionMedia,
      vendedores_para_cubrir_costos: Math.max(1, vendedores),
    };
  }

  /** Economia de escala (8.7): costo fijo de plataforma por pedido (C$ 4,0 -> 0,95). */
  economiaDeEscala(filas: FilaProyeccion[]): EconomiaDeEscala {
    const m1 = filas[0];
    const m6 = filas.find((f) => f.mes === 6);
    const m12 = filas.find((f) => f.mes === 12) ?? filas[filas.length - 1];
    const inicio = m1?.costo_fijo_por_pedido_cents ?? 0;
    const fin = m12?.costo_fijo_por_pedido_cents ?? 0;
    return {
      costo_fijo_por_pedido_inicio_cents: inicio,
      costo_fijo_por_pedido_mes6_cents: m6?.costo_fijo_por_pedido_cents ?? 0,
      costo_fijo_por_pedido_fin_cents: fin,
      multiplicador_escala: fin > 0 ? Math.round((inicio / fin) * 100) / 100 : 0,
    };
  }

  /**
   * Matriz de sensibilidad comision x demanda (8.10, tabla 8.12): flujo del
   * mes 12 para tasas de comision de 8/10/12/14 % y variaciones de -30 %/base/
   * +15 % del GMV proyectado. Todos los escenarios mantienen flujo positivo.
   */
  sensibilidadComisionGmv(supuestos: SupuestosProyeccion): SensibilidadComisionGmv[] {
    const tasas = [0.08, 0.1, 0.12, 0.14];
    const variaciones = [-0.3, 0, 0.15];
    const ultimoMes = supuestos.pedidos_por_vendedor.length;
    return tasas.map((tasa) => ({
      tasa_comision: tasa,
      flujo_mes12_cents: variaciones.map((v) => {
        const modificados: SupuestosProyeccion = {
          ...supuestos,
          comision_tasa: tasa,
          pedidos_por_vendedor: supuestos.pedidos_por_vendedor.map((p) => p * (1 + v)),
        };
        const { filas } = this.calcular(modificados);
        return { variacion_gmv: v, flujo_cents: filas[ultimoMes - 1].flujo_cents };
      }),
    }));
  }

  /** Sensibilidad del VAN al costo de oportunidad (8.11, tabla 8.12): 12-30 % anual. */
  sensibilidadVan(filas: FilaProyeccion[], supuestos: SupuestosProyeccion): SensibilidadVan[] {
    const inversion = supuestos.inversion_inicial_cents;
    return [0.12, 0.18, 0.24, 0.3].map((tasaAnual) => {
      const tasaMensual = tasaAnual / 12;
      const van =
        filas.reduce((s, f, i) => s + f.flujo_cents / Math.pow(1 + tasaMensual, i + 1), 0) - inversion;
      return {
        tasa_anual: tasaAnual,
        tasa_mensual: tasaMensual,
        van_cents: Math.round(van),
        relacion_van_inversion: inversion > 0 ? Math.round((van / inversion) * 10) / 10 : 0,
      };
    });
  }

  /** Payback por escenario (8.11, tabla 8.12): base / pesimista (-30 %) / peor combinado. */
  paybackPorEscenario(supuestos: SupuestosProyeccion): PaybackPorEscenario[] {
    const conEscenario = (
      escenario: PaybackPorEscenario['escenario'],
      factorPedidos: number,
      tasa: number,
    ): PaybackPorEscenario => {
      const modificados: SupuestosProyeccion = {
        ...supuestos,
        comision_tasa: tasa,
        pedidos_por_vendedor: supuestos.pedidos_por_vendedor.map((p) => p * factorPedidos),
      };
      const { filas } = this.calcular(modificados);
      const inversion = supuestos.inversion_inicial_cents;
      const payback = filas.find((f) => f.flujo_acumulado_cents >= inversion)?.mes ?? null;
      const acumulado = payback !== null ? filas[payback - 1].flujo_acumulado_cents : 0;
      return { escenario, descripcion: '', flujo_acumulado_cents: acumulado, payback_mes: payback };
    };

    const base = conEscenario('base', 1, supuestos.comision_tasa);
    const pesimista = conEscenario('pesimista', 0.7, supuestos.comision_tasa);
    const peor = conEscenario('peor_combinado', 0.7, 0.08);
    const descripcion: Record<PaybackPorEscenario['escenario'], string> = {
      base: 'Recupera la inversion en el segundo mes.',
      pesimista: 'Aun con 30 % menos de pedidos, caja positiva continua.',
      peor_combinado: 'Con la comision minima y demanda baja, se recupera en el mes 4.',
    };
    return [base, pesimista, peor].map((e) => ({ ...e, descripcion: descripcion[e.escenario] }));
  }

  /**
   * Supuestos del segundo anio (8.9): entrada de 3 vendedores/mes (78 -> 111),
   * pedidos por vendedor de 42 a 43, costos fijos de C$ 3 000 todo el anio.
   * Reproduce la tabla 8.9: 48 249 pedidos, C$ 21 712 050 de GMV, C$ 2 605 446
   * de ingresos y flujo neto de +C$ 2 569 446 (margen 98,6 %).
   */
  presetSegundoAnio(): SupuestosProyeccion {
    return {
      horizonte_meses: 12,
      vendedores_iniciales: 78,
      entrada_vendedores_mes: 3,
      churn_tasa: 0,
      pedidos_por_vendedor: [42, 42, 42, 42, 42, 42, 43, 43, 43, 43, 43, 43],
      ticket_promedio_cents: 45000,
      comision_tasa: 0.12,
      costos_fijos_cents: 300000,
      costos_fijos_desde_mes7_cents: 300000,
      inversion_inicial_cents: 5850000,
      tasa_descuento_mensual: 0.015,
    };
  }

  /**
   * Plan a 24 meses: anio 1 (tabla 8.6) + anio 2 indicativo (8.9). Entrega la
   * cuenta de resultados por anio (tabla 8.11), el ROI (~63x), el acumulado
   * (GMV ~C$ 31,1 M) y el cierre del mes 24 (+52 % sobre el mes 12).
   */
  planBienal(supuestosAnio1: SupuestosProyeccion): PlanBienal {
    const anio1 = this.calcular(supuestosAnio1);
    const anio2 = this.calcular(this.presetSegundoAnio());
    const filas = [...anio1.filas, ...anio2.filas];
    const inversion = supuestosAnio1.inversion_inicial_cents;
    const resumen = (a: number, f: FilaProyeccion[]): ResumenAnual => {
      const gmv = f.reduce((s, x) => s + x.gmv_cents, 0);
      const ingresos = f.reduce((s, x) => s + x.ingresos_cents, 0);
      const costos = f.reduce((s, x) => s + x.costos_cents, 0);
      const flujo = f.reduce((s, x) => s + x.flujo_cents, 0);
      return { anio: a, gmv_cents: gmv, ingresos_cents: ingresos, costos_cents: costos, flujo_cents: flujo, margen_neto: ingresos > 0 ? flujo / ingresos : 0 };
    };
    const anios = [resumen(1, anio1.filas), resumen(2, anio2.filas)];
    const flujoTotal = anios.reduce((s, a) => s + a.flujo_cents, 0);
    const mes24 = filas[23];
    return {
      inversion_inicial_cents: inversion,
      anios,
      filas,
      acumulado_24_meses: {
        gmv_cents: anios.reduce((s, a) => s + a.gmv_cents, 0),
        ingresos_cents: anios.reduce((s, a) => s + a.ingresos_cents, 0),
        costos_cents: anios.reduce((s, a) => s + a.costos_cents, 0),
        flujo_cents: flujoTotal,
      },
      roi: inversion > 0 ? flujoTotal / inversion : 0,
      mes_24: {
        gmv_cents: mes24.gmv_cents,
        incremento_vs_mes12: anio1.filas[11].gmv_cents > 0 ? mes24.gmv_cents / anio1.filas[11].gmv_cents - 1 : 0,
      },
      indicadores: this.indicadores(filas, { ...supuestosAnio1, horizonte_meses: 24 }),
    };
  }

  /** TIR mensual por bisqueda binaria en [0, 5] (500 %/mes max). */
  private tirMensual(filas: FilaProyeccion[], inversion: number): number | null {
    const f = (r: number): number =>
      filas.reduce((s, fila, i) => s + fila.flujo_cents / Math.pow(1 + r, i + 1), 0) - inversion;
    if (f(0) <= 0) return 0;
    if (f(5) > 0) {
      // los flujos son tan positivos que el VAN no se anula: TIR no estable
      return null;
    }
    let bajo = 0;
    let alto = 5;
    for (let i = 0; i < 200; i += 1) {
      const medio = (bajo + alto) / 2;
      if (f(medio) > 0) bajo = medio;
      else alto = medio;
    }
    return (bajo + alto) / 2;
  }

  async crear(nombre: string, supuestos: SupuestosProyeccion): Promise<Proyeccion> {
    const { filas, totales } = this.calcular(supuestos);
    const indicadores = this.indicadores(filas, supuestos);
    const fila = await this.pg.queryOne<Omit<Proyeccion, 'supuestos' | 'filas' | 'totales' | 'indicadores'>>(
      `INSERT INTO finance.proyecciones (nombre, horizonte_meses, supuestos, filas, totales, inversion_inicial_cents)
       VALUES ($1, $2, $3, $4, $5, $6)
       RETURNING id, nombre, horizonte_meses, inversion_inicial_cents, creado_en`,
      [
        nombre,
        supuestos.horizonte_meses,
        JSON.stringify(supuestos),
        JSON.stringify(filas),
        JSON.stringify(totales),
        supuestos.inversion_inicial_cents,
      ],
    );
    if (!fila) throw new DomainError('PROYECCION_FALLIDA', 'No se pudo guardar la proyeccion.');
    this.logger.info({ msg: 'Proyeccion creada', id: fila.id, nombre, meses: supuestos.horizonte_meses });
    return {
      ...fila,
      supuestos,
      filas,
      totales,
      indicadores,
    };
  }

  async listar(): Promise<{ id: string; nombre: string; horizonte_meses: number; creado_en: string }[]> {
    return this.pg.query(
      `SELECT id, nombre, horizonte_meses, creado_en FROM finance.proyecciones ORDER BY creado_en DESC`,
    );
  }

  async obtener(id: string): Promise<Proyeccion | null> {
    const fila = await this.pg.queryOne<Omit<Proyeccion, 'supuestos' | 'filas' | 'totales' | 'indicadores'> & {
      supuestos: string;
      filas: string;
      totales: string;
    }>(
      `SELECT id, nombre, horizonte_meses, supuestos, filas, totales, inversion_inicial_cents, creado_en
       FROM finance.proyecciones WHERE id = $1`,
      [id],
    );
    if (!fila) return null;
    const supuestos = typeof fila.supuestos === 'string' ? JSON.parse(fila.supuestos) : fila.supuestos;
    const filas = typeof fila.filas === 'string' ? JSON.parse(fila.filas) : fila.filas;
    const totalesObj = typeof fila.totales === 'string' ? JSON.parse(fila.totales) : fila.totales;
    return {
      id: fila.id,
      nombre: fila.nombre,
      horizonte_meses: fila.horizonte_meses,
      supuestos,
      filas,
      totales: totalesObj,
      inversion_inicial_cents: fila.inversion_inicial_cents,
      indicadores: this.indicadores(filas as FilaProyeccion[], supuestos as SupuestosProyeccion),
      creado_en: fila.creado_en,
    };
  }

  /** Censo de ventas: formatea centavos a C$ con dos decimales (doc 5.7). */
  formatear(centavos: number): string {
    return Money.desdeCentavos(centavos).string();
  }
}