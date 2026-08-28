import { Injectable } from '@nestjs/common';
import { PgService, Money, Logger } from '@core/shared';

export interface KpisFinancieros {
  periodo: string;
  gmv_cents: number;
  gmv: string;
  pedidos_pagados: number;
  pedidos_entregados: number;
  ingresos_comisiones_cents: number;
  ingresos_comisiones: string;
  devoluciones: number;
  vendedores_activos: number;
  pedidos_por_vendedor: number;
  activacion_primeros_15_dias: number | null; // null si la identidad no responde
  tasa_devoluciones: number;
  churn_mensual: number;
  /** KPI de embudo (8.8): checkouts completados / carritos iniciados. null sin datos. */
  conversion_carrito_checkout: number | null;
  /** KPI de caja (8.8): cortes quincenales (RN-07) pagados a tiempo. null sin datos. */
  liquidacion_a_tiempo: number | null;
}

/**
 * KPIs del tablero financiero (tabla 8.8) alimentados por los eventos del bus
 * (doc 8.8: misma fuente de verdad que el sistema, sin reconciliacion manual):
 * las metricas se materializan en finance.metricas_* desde el consumer.
 */
@Injectable()
export class KpisService {
  private readonly identidadUrl = process.env.IDENTITY_SERVICE_URL ?? 'http://identity-service:3001';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly logger = Logger.create('finance.kpis');

  constructor(private readonly pg: PgService) {}

  async kpis(mes?: string): Promise<KpisFinancieros> {
    const periodo = mes ?? new Date().toISOString().slice(0, 10); // default: mes actual
    const inicioMes = `${periodo.slice(0, 7)}-01`;

    const periodos = await this.leerMetricasMensuales();
    const gmv = periodos.get(`${inicioMes}|gmv`)?.monto ?? 0;
    const pedidosPagados = periodos.get(`${inicioMes}|pedidos`)?.cantidad ?? 0;
    const entregados = periodos.get(`${inicioMes}|entregados`)?.cantidad ?? 0;
    const comisiones = periodos.get(`${inicioMes}|ingresos_comisiones`)?.monto ?? 0;
    const devoluciones = periodos.get(`${inicioMes}|devoluciones`)?.cantidad ?? 0;

    const vendedoresActivos = await this.vendedoresActivos30Dias();
    const pedidosPorVendedor = vendedoresActivos > 0 ? pedidosPagados / vendedoresActivos : 0;
    const tasaDevoluciones = entregados > 0 ? devoluciones / entregados : 0;

    const [activacion, churn] = await Promise.all([this.activacion15Dias(), this.churnMensual()]);
    const conversion = this.conversionEmbudo(periodos, inicioMes);
    const liquidacion = this.liquidacionATiempo(periodos, inicioMes);
    this.logger.info({ msg: 'KPIs financieros calculados', periodo: inicioMes, gmv_cents: gmv });

    return {
      periodo: inicioMes,
      gmv_cents: gmv,
      gmv: Money.desdeCentavos(gmv).string(),
      pedidos_pagados: pedidosPagados,
      pedidos_entregados: entregados,
      ingresos_comisiones_cents: comisiones,
      ingresos_comisiones: Money.desdeCentavos(comisiones).string(),
      devoluciones,
      vendedores_activos: vendedoresActivos,
      pedidos_por_vendedor: Math.round(pedidosPorVendedor * 100) / 100,
      activacion_primeros_15_dias: activacion,
      tasa_devoluciones: Math.round(tasaDevoluciones * 10000) / 10000,
      churn_mensual: Math.round(churn * 10000) / 10000,
      conversion_carrito_checkout: conversion,
      liquidacion_a_tiempo: liquidacion,
    };
  }

  /**
   * Embudo carrito -> checkout (tabla 8.8): checkouts completados / carritos
   * iniciados. Fuente: eventos del bus (8.8); null hasta que existan datos
   * (el embudo se audita semanal, tabla 8.10).
   */
  conversionEmbudo(
    periodos: Map<string, { monto: number; cantidad: number }>,
    inicioMes: string,
  ): number | null {
    const carritos = periodos.get(`${inicioMes}|carritos_iniciados`)?.cantidad ?? 0;
    const checkouts = periodos.get(`${inicioMes}|checkouts_completados`)?.cantidad ?? 0;
    if (carritos <= 0 || checkouts <= 0) return null;
    return Math.round((checkouts / carritos) * 10000) / 10000;
  }

  /**
   * Liquidacion a tiempo (RN-07, tabla 8.8): cortes quincenales (dias 1 y 15)
   * pagados en la fecha pactada. Fuente: metricas del corte; null sin datos.
   */
  liquidacionATiempo(
    periodos: Map<string, { monto: number; cantidad: number }>,
    inicioMes: string,
  ): number | null {
    const aTiempo = periodos.get(`${inicioMes}|liquidaciones_a_tiempo`)?.cantidad ?? 0;
    const esperados = periodos.get(`${inicioMes}|cortes_esperados`)?.cantidad ?? 0;
    if (esperados <= 0) return null;
    return Math.round((aTiempo / esperados) * 10000) / 10000;
  }

  private async leerMetricasMensuales(): Promise<Map<string, { monto: number; cantidad: number }>> {
    const filas = await this.pg.query<{ periodo: string; tipo: string; monto_cents: number; cantidad: number }>(
      `SELECT TO_CHAR(periodo, 'YYYY-MM-DD') AS periodo, tipo, monto_cents, cantidad
       FROM finance.metricas_mensuales`,
    );
    const mapa = new Map<string, { monto: number; cantidad: number }>();
    for (const f of filas) mapa.set(`${f.periodo}|${f.tipo}`, { monto: f.monto_cents, cantidad: f.cantidad });
    return mapa;
  }

  /** Vendedores con al menos 1 venta en los ultimos 30 dias (tabla 8.8). */
  private async vendedoresActivos30Dias(): Promise<number> {
    const fila = await this.pg.queryOne<{ n: number }>(
      `SELECT COUNT(DISTINCT vendedor_id)::int AS n
       FROM finance.metricas_vendedores
       WHERE ultima_venta_en >= NOW() - INTERVAL '30 days'`,
    );
    return fila?.n ?? 0;
  }

  /** 1.a venta en <= 15 dias desde el registro / vendedores registrados. */
  private async activacion15Dias(): Promise<number | null> {
    try {
      const res = await fetch(`${this.identidadUrl}/internal/vendedores`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) return null;
      const vendedores = (await res.json()) as { id: string; creado_en: string }[];
      if (!Array.isArray(vendedores) || vendedores.length === 0) return null;

      const periodos: Record<string, { ventas: number; primera_venta_en: string | null }> = {};
      const filas = await this.pg.query<{
        vendedor_id: string;
        ventas: number;
        primera_venta_en: string | null;
      }>(`SELECT vendedor_id, ventas, TO_CHAR(primera_venta_en, 'YYYY-MM-DD"T"HH24:MI:SS') AS primera_venta_en
          FROM finance.metricas_vendedores`);
      for (const f of filas) {
        periodos[f.vendedor_id] = { ventas: f.ventas, primera_venta_en: f.primera_venta_en };
      }

      let registrados = 0;
      let activados = 0;
      for (const v of vendedores) {
        const datos = periodos[v.id];
        if (!datos || datos.ventas === 0 || !datos.primera_venta_en || !v.creado_en) continue;
        registrados += 1;
        const registroMs = new Date(v.creado_en).getTime();
        const primeraVentaMs = new Date(datos.primera_venta_en).getTime();
        if (primeraVentaMs - registroMs <= 15 * 24 * 60 * 60 * 1000) activados += 1;
      }
      return registrados > 0 ? activados / registrados : null;
    } catch (err) {
      this.logger.warn({ msg: 'No se pudo computar la activacion (identidad no disponible)', err: String(err) });
      return null;
    }
  }

  /** Vendedores inactivos / vendedores activos del periodo anterior (8.8). */
  private async churnMensual(): Promise<number> {
    const prev = await this.pg.query<{ vendedor_id: string }>(
      `SELECT DISTINCT vendedor_id FROM finance.metricas_vendedores
       WHERE ultima_venta_en >= NOW() - INTERVAL '60 days' AND ultima_venta_en < NOW() - INTERVAL '30 days'`,
    );
    if (prev.length === 0) return 0;
    const activos = await this.pg.query<{ vendedor_id: string }>(
      `SELECT DISTINCT vendedor_id FROM finance.metricas_vendedores
       WHERE ultima_venta_en >= NOW() - INTERVAL '30 days'`,
    );
    const activosSet = new Set(activos.map((a) => a.vendedor_id));
    const inactivos = prev.filter((p) => !activosSet.has(p.vendedor_id)).length;
    return inactivos / prev.length;
  }

  /** Censo de ventas de un vendedor para su declaracion (cap. 4.4). */
  /** GMV real del mes (para el desvio frente a la proyeccion, tabla 8.10). */
  async leerGMVReal(mes: string): Promise<number | null> {
    const inicioMes = `${mes.slice(0, 7)}-01`;
    const periodos = await this.leerMetricasMensuales();
    return periodos.get(`${inicioMes}|gmv`)?.monto ?? null;
  }

  /** Resumen de negocios de un vendedor para su declaracion (cap. 4.4). */
  async resumenDelVendedor(vendedorId: string, mes?: string): Promise<{
    vendedor_id: string;
    periodo: string;
    ventas: number;
    monto_cents: number;
    comision_cents: number;
    devoluciones: number;
  } | null> {
    const periodo = mes ?? new Date().toISOString().slice(0, 7);
    const fila = await this.pg.queryOne<{
      vendedor_id: string;
      ventas: number;
      monto_cents: number;
      comision_cents: number;
      devoluciones: number;
    }>(
      `SELECT vendedor_id,
              SUM(ventas)::int AS ventas,
              SUM(monto_cents)::int AS monto_cents,
              SUM(comision_cents)::int AS comision_cents,
              SUM(devoluciones)::int AS devoluciones
       FROM finance.metricas_vendedores
       WHERE vendedor_id = $1 AND TO_CHAR(periodo, 'YYYY-MM') = $2
       GROUP BY vendedor_id`,
      [vendedorId, periodo],
    );
    if (!fila || fila.ventas === 0) return null;
    return { ...fila, periodo };
  }
}