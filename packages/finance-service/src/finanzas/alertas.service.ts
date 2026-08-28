import { Injectable } from '@nestjs/common';
import { Logger } from '@core/shared';
import { KpisService, KpisFinancieros } from './kpis.service';
import { ProyeccionesService } from './proyecciones.service';

/**
 * Jerarquia del tablero de metricas con reglas de alerta (tabla 8.10 del
 * informe): cada capa tiene su frecuencia de evaluacion, su umbral y una
 * accion concreta. Las reglas se evaluan contra los KPIs (que derivan de los
 * eventos del bus, doc 8.8) y la proyeccion guardada mas reciente (8.6).
 *
 * Capas (doc 8.8 "Jerarquia del tablero y reglas de alerta"):
 *  - North-star (mensual): desvio del GMV > 20 % frente a la proyeccion
 *    -> revisar supuestos (8.12).
 *  - Adquisicion (quincenal): activacion < 50 % -> onboarding asistido en 7 dias.
 *  - Operacion (semanal): last-mile > C$ 45 o devoluciones > 5 %
 *    -> contingencia (cap. 5.2).
 *  - Caja (quincenal): cualquier retraso en liquidacion -> alerta inmediata.
 */
@Injectable()
export class AlertasService {
  private readonly logger = Logger.create('finance.alertas');

  constructor(
    private readonly kpis: KpisService,
    private readonly proyecciones: ProyeccionesService,
  ) {}

  /** Capas que deben evaluarse hoy segun el ritual de revision (doc 8.8). */
  capasDebidas(hoy: Date = new Date()): string[] {
    const dia = hoy.getDate();
    const esLunes = hoy.getDay() === 1;
    const debidas: string[] = [];
    if (esLunes) debidas.push('operacion'); // semanal: embudo y activacion
    if (dia === 1 || dia === 15) {
      debidas.push('adquisicion'); // quincenal
      debidas.push('caja'); // quincenal
    }
    if (dia === 1) debidas.push('north_star'); // mensual
    return debidas;
  }

  /**
   * Tablero completo: KPIs + reglas de alerta de todas las capas con su
   * estado (OK / ALERTA / SIN_DATO) y la accion recomendada del informe.
   */
  async tablero(opciones?: {
    mes?: string;
    costo_entrega_por_pedido_cents?: number;
    hoy?: Date;
  }): Promise<{
    periodo: string;
    kpis: KpisFinancieros;
    alertas: TableroAlerta[];
  }> {
    const mes = opciones?.mes;
    const kpis = await this.kpis.kpis(mes);
    const hoy = opciones?.hoy ?? new Date();
    const debidas = this.capasDebidas(hoy);
    const proyeccion = await this.ultimaProyeccion();

    const alertas: TableroAlerta[] = [
      this.regla({
        capa: 'north_star',
        frecuencia: 'MENSUAL',
        indicador: 'GMV vs proyeccion (8.6)',
        evaluable: debidas.includes('north_star'),
        valor: proyeccion?.desvio_gmv ?? null,
        umbral: 'desvio > 20 %',
        regla: proyeccion === null || proyeccion.desvio_gmv === null
          ? 'SIN_DATO'
          : proyeccion.desvio_gmv > 0.2
            ? 'ALERTA'
            : 'OK',
        accion: 'Revisar supuestos (8.12).',
        detalle: proyeccion
          ? `GMV real ${kpis.gmv} vs proyectado ${proyeccion.gmv_proyectado} del periodo.`
          : 'No hay proyeccion guardada para comparar.',
      }),
      this.regla({
        capa: 'adquisicion',
        frecuencia: 'QUINCENAL',
        indicador: 'Activacion de vendedores (15 dias)',
        evaluable: debidas.includes('adquisicion'),
        valor: kpis.activacion_primeros_15_dias,
        umbral: '< 50 %',
        regla:
          kpis.activacion_primeros_15_dias === null
            ? 'SIN_DATO'
            : kpis.activacion_primeros_15_dias < 0.5
              ? 'ALERTA'
              : 'OK',
        accion: 'Onboarding asistido en 7 dias.',
      }),
      this.regla({
        capa: 'operacion',
        frecuencia: 'SEMANAL',
        indicador: 'Tasa de devoluciones',
        evaluable: debidas.includes('operacion'),
        valor: kpis.tasa_devoluciones,
        umbral: '> 5 %',
        regla: kpis.tasa_devoluciones > 0.05 ? 'ALERTA' : 'OK',
        accion: 'Contingencia (cap. 5.2).',
      }),
      this.regla({
        capa: 'operacion',
        frecuencia: 'SEMANAL',
        indicador: 'Costo last-mile por pedido',
        evaluable: debidas.includes('operacion'),
        valor:
          opciones?.costo_entrega_por_pedido_cents === undefined
            ? null
            : opciones.costo_entrega_por_pedido_cents / 100,
        umbral: '> C$ 45',
        regla:
          opciones?.costo_entrega_por_pedido_cents === undefined
            ? 'SIN_DATO'
            : opciones.costo_entrega_por_pedido_cents > 4500
              ? 'ALERTA'
              : 'OK',
        accion: 'Tarifa de entrega al comprador o reduccion de zona (8.12).',
        detalle: 'Se evalua cuando el piloto provee el costo real del courier (8.12).',
      }),
      this.regla({
        capa: 'caja',
        frecuencia: 'QUINCENAL',
        indicador: 'Liquidacion a tiempo (RN-07)',
        evaluable: debidas.includes('caja'),
        valor: kpis.liquidacion_a_tiempo,
        umbral: '100 %',
        regla:
          kpis.liquidacion_a_tiempo === null
            ? 'SIN_DATO'
            : kpis.liquidacion_a_tiempo < 1
              ? 'ALERTA'
              : 'OK',
        accion: 'Alerta inmediata: cualquier retraso en liquidacion.',
        detalle:
          kpis.liquidacion_a_tiempo === null
            ? 'Sin eventos de corte aun; se conecta cuando el corte quincenal publique sus metricas.'
            : `Cortes pagados a tiempo: ${Math.round((kpis.liquidacion_a_tiempo ?? 0) * 100)} %.`,
      }),
    ];

    this.logger.info({ msg: 'Tablero evaluado', periodo: kpis.periodo, alertas: alertas.length });
    return { periodo: kpis.periodo, kpis, alertas };
  }

  /** Desvio del GMV real frente a la ultima proyeccion guardada (8.6). */
  private async ultimaProyeccion(): Promise<{
    gmv_proyectado: string;
    desvio_gmv: number | null;
  } | null> {
    const listado = await this.proyecciones.listar();
    if (listado.length === 0) return null;
    const proyeccion = await this.proyecciones.obtener(listado[0].id);
    if (!proyeccion) return null;
    const periodo = new Date().toISOString().slice(0, 7);
    const mesNumero = Number(periodo.slice(5, 7));
    const fila = proyeccion.filas.find((f) => f.mes === mesNumero);
    if (!fila) return null;

    const gmvReal = await this.kpis.leerGMVReal(periodo);
    if (gmvReal === null) return { gmv_proyectado: '', desvio_gmv: null };
    const desvio = (gmvReal - fila.gmv_cents) / fila.gmv_cents;
    return {
      gmv_proyectado: `${(fila.gmv_cents / 100).toLocaleString('es-NI', { minimumFractionDigits: 2 })} C$`,
      desvio_gmv: Math.round(desvio * 10000) / 10000,
    };
  }

  private regla(campos: {
    capa: string;
    frecuencia: string;
    indicador: string;
    evaluable: boolean;
    valor: number | null;
    umbral: string;
    regla: string;
    accion: string;
    detalle?: string;
  }): TableroAlerta {
    return {
      capa: campos.capa,
      frecuencia: campos.frecuencia,
      indicador: campos.indicador,
      estado:
        campos.regla === 'ALERTA' ? 'ALERTA' : campos.regla === 'SIN_DATO' ? 'SIN_DATO' : 'OK',
      umbral: campos.umbral,
      valor: campos.valor,
      accion: campos.accion,
      detalle: campos.detalle,
      fecha_esperada: campos.frecuencia,
    };
  }
}

export interface TableroAlerta {
  capa: string;
  frecuencia: string;
  indicador: string;
  estado: 'OK' | 'ALERTA' | 'SIN_DATO';
  umbral: string;
  valor: number | null;
  accion: string;
  detalle?: string;
  fecha_esperada: string;
}