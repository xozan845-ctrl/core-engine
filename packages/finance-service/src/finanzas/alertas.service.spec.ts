import { AlertasService } from './alertas.service';
import { KpisFinancieros } from './kpis.service';

/**
 * Tablero financiero (8.10): reglas de alerta por capa con jerarquia,
 * frecuencias del ritual de revision y acciones del informe.
 */
describe('AlertasService (tablero: reglas de alerta por capa)', () => {
  const mesActual = new Date().getMonth() + 1; // el desvio compara contra el mes real
  const filaMesActual = (gmv_cents: number) => ({ mes: mesActual, gmv_cents });
  const kpisBase = (sobre: Partial<KpisFinancieros> = {}): KpisFinancieros => ({
    periodo: '2026-08',
    gmv_cents: 120000000,
    gmv: 'C$ 1 200 000,00',
    pedidos_pagados: 2600,
    pedidos_entregados: 2500,
    ingresos_comisiones_cents: 14400000,
    ingresos_comisiones: 'C$ 144 000,00',
    devoluciones: 52,
    vendedores_activos: 74,
    pedidos_por_vendedor: 35.1,
    tasa_devoluciones: 0.02,
    churn_mensual: 0,
    activacion_primeros_15_dias: 0.6,
    conversion_carrito_checkout: 0.4,
    liquidacion_a_tiempo: 1,
    ...sobre,
  } as KpisFinancieros);

  const crear = (opciones: {
    kpis?: Partial<KpisFinancieros>;
    gmvReal?: number | null;
    filaProyectada?: { mes: number; gmv_cents: number };
    conProyeccion?: boolean;
  }) => {
    const kpis = {
      kpis: jest.fn().mockResolvedValue(kpisBase(opciones.kpis)),
      leerGMVReal: jest.fn().mockResolvedValue(opciones.gmvReal ?? null),
    };
    const proyecciones = {
      listar: jest.fn().mockResolvedValue(opciones.conProyeccion === false ? [] : [{ id: 'p1' }]),
      obtener: jest.fn().mockResolvedValue(
        opciones.filaProyectada ? { filas: [opciones.filaProyectada] } : { filas: [] },
      ),
    };
    return { servicio: new AlertasService(kpis as never, proyecciones as never), kpis, proyecciones };
  };

  it('ritual de revision (8.8): lunes -> operacion; dia 15 -> adquisicion+caja; dia 1 -> todo', () => {
    const servicio = new AlertasService({} as never, {} as never);
    expect(servicio.capasDebidas(new Date(2026, 7, 3))).toEqual(['operacion']); // lunes
    expect(servicio.capasDebidas(new Date(2026, 7, 15))).toEqual(['adquisicion', 'caja']);
    const diaUno = servicio.capasDebidas(new Date(2026, 7, 1));
    expect(diaUno).toContain('north_star');
    expect(diaUno).toContain('adquisicion');
    expect(diaUno).toContain('caja');
  });

  it('north-star: desvio de GMV > 20 % frente a la proyeccion -> ALERTA y revisar supuestos (8.12)', async () => {
    // 30 % por encima de lo proyectado (fila del mes 8 vs gmv real)
    const { servicio } = crear({
      gmvReal: 130000000,
      filaProyectada: filaMesActual(100000000),
    });
    const { alertas } = await servicio.tablero({ hoy: new Date(2026, 7, 1) });
    const ns = alertas.find((a) => a.capa === 'north_star')!;
    expect(ns.estado).toBe('ALERTA');
    expect(ns.umbral).toBe('desvio > 20 %');
    expect(ns.valor).toBeCloseTo(0.3, 4);
    expect(ns.accion).toContain('Revisar supuestos');
  });

  it('north-star: sin proyeccion guardada -> SIN_DATO', async () => {
    const { servicio } = crear({ conProyeccion: false });
    const { alertas } = await servicio.tablero({ hoy: new Date(2026, 7, 1) });
    expect(alertas.find((a) => a.capa === 'north_star')!.estado).toBe('SIN_DATO');
  });

  it('adquisicion: activacion < 50 % -> onboarding asistido en 7 dias', async () => {
    const { servicio } = crear({
      kpis: { activacion_primeros_15_dias: 0.4 },
      gmvReal: 100000000,
      filaProyectada: filaMesActual(100000000),
    });
    const { alertas } = await servicio.tablero({ hoy: new Date(2026, 7, 15) });
    const adq = alertas.find((a) => a.capa === 'adquisicion')!;
    expect(adq.estado).toBe('ALERTA');
    expect(adq.accion).toContain('Onboarding asistido en 7 dias');
  });

  it('operacion: devoluciones > 5 % -> contingencia (cap. 5.2)', async () => {
    const { servicio } = crear({
      kpis: { tasa_devoluciones: 0.06 },
      gmvReal: 100000000,
      filaProyectada: filaMesActual(100000000),
    });
    const { alertas } = await servicio.tablero({ hoy: new Date(2026, 7, 3) });
    const op = alertas.find((a) => a.capa === 'operacion' && a.indicador === 'Tasa de devoluciones')!;
    expect(op.estado).toBe('ALERTA');
    expect(op.accion).toContain('Contingencia');
  });

  it('operacion: last-mile > C$ 45 -> ALERTA; sin dato -> SIN_DATO', async () => {
    const { servicio } = crear({
      gmvReal: 100000000,
      filaProyectada: filaMesActual(100000000),
    });
    const conDato = await servicio.tablero({
      hoy: new Date(2026, 7, 3),
      costo_entrega_por_pedido_cents: 5000, // C$ 50
    });
    expect(conDato.alertas.find((a) => a.indicador === 'Costo last-mile por pedido')!.estado).toBe('ALERTA');
    const sinDato = await servicio.tablero({ hoy: new Date(2026, 7, 3) });
    expect(sinDato.alertas.find((a) => a.indicador === 'Costo last-mile por pedido')!.estado).toBe('SIN_DATO');
  });

  it('caja: retraso en la liquidacion (RN-07) -> ALERTA inmediata; sin cortes -> SIN_DATO', async () => {
    const conProy = { gmvReal: 100000000, filaProyectada: filaMesActual(100000000) };
    const { servicio } = crear({ kpis: { liquidacion_a_tiempo: 0.85 }, ...conProy });
    const res = await servicio.tablero({ hoy: new Date(2026, 7, 15) });
    const caja = res.alertas.find((a) => a.capa === 'caja')!;
    expect(caja.estado).toBe('ALERTA');
    expect(caja.accion).toContain('Alerta inmediata');

    const sinCortes = crear({ kpis: { liquidacion_a_tiempo: null }, ...conProy });
    const res2 = await sinCortes.servicio.tablero({ hoy: new Date(2026, 7, 15) });
    expect(res2.alertas.find((a) => a.capa === 'caja')!.estado).toBe('SIN_DATO');
  });

  it('todo dentro de umbrales -> todas las capas OK y KPIs presentes', async () => {
    const { servicio } = crear({
      kpis: { activacion_primeros_15_dias: 0.6, tasa_devoluciones: 0.02, liquidacion_a_tiempo: 1 },
      gmvReal: 105000000,
      filaProyectada: filaMesActual(100000000),
    });
    const { alertas, kpis } = await servicio.tablero({
      hoy: new Date(2026, 7, 3),
      costo_entrega_por_pedido_cents: 4000, // C$ 40
    });
    expect(kpis.periodo).toBe('2026-08');
    const sinAlerta = alertas.filter((a) => a.estado === 'ALERTA');
    expect(sinAlerta).toHaveLength(0);
    expect(alertas.every((a) => a.frecuencia)).toBe(true);
  });
});