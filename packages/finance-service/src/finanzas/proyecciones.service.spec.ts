import { ProyeccionesService, SupuestosProyeccion } from './proyecciones.service';

/**
 * Valida el modelo de calculo del cap. 8 contra la tabla 8.9 (proyeccion
 * mensual del primer ano) y los indicadores de las tablas 8.11-8.13.
 */
describe('ProyeccionesService (cap. 8: modelo auditado del informe)', () => {
  const servicio = (() => {
    const fake = {
      pg: { query: jest.fn(), queryOne: jest.fn(), transaccion: jest.fn() },
      logger: { info: jest.fn(), warn: jest.fn(), error: jest.fn() },
    };
    return new ProyeccionesService(fake.pg as never);
  })();

  const supuestosDoc: SupuestosProyeccion = {
    horizonte_meses: 12,
    vendedores_iniciales: 20,
    entrada_vendedores_mes: 5,
    churn_tasa: 0,
    pedidos_por_vendedor: [25, 27, 29, 31, 33, 35, 37, 38, 39, 40, 41, 42],
    ticket_promedio_cents: 45000, // C$ 450
    comision_tasa: 0.12, // RN-04
    costos_fijos_cents: 200000, // C$ 2 000 (meses 1-6)
    costos_fijos_desde_mes7_cents: 300000, // C$ 3 000 (meses 7-12)
    inversion_inicial_cents: 5850000, // C$ 58 500 (tabla 8.14)
    tasa_descuento_mensual: 0.015, // 18 % anual / 12
  };

  it('reproduce exactamente la tabla 8.9 (12 meses)', () => {
    const { filas } = servicio.calcular(supuestosDoc);
    const esperado = [
      [20, 500, 22500000, 2700000, 200000, 2500000],
      [25, 675, 30375000, 3645000, 200000, 3445000],
      [30, 870, 39150000, 4698000, 200000, 4498000],
      [35, 1085, 48825000, 5859000, 200000, 5659000],
      [40, 1320, 59400000, 7128000, 200000, 6928000],
      [45, 1575, 70875000, 8505000, 200000, 8305000],
      [50, 1850, 83250000, 9990000, 300000, 9690000],
      [55, 2090, 94050000, 11286000, 300000, 10986000],
      [60, 2340, 105300000, 12636000, 300000, 12336000],
      [65, 2600, 117000000, 14040000, 300000, 13740000],
      [70, 2870, 129150000, 15498000, 300000, 15198000],
      [75, 3150, 141750000, 17010000, 300000, 16710000],
    ];
    expect(filas).toHaveLength(12);
    filas.forEach((f, i) => {
      expect([f.vendedores, f.pedidos, f.gmv_cents, f.ingresos_cents, f.costos_cents, f.flujo_cents]).toEqual(
        esperado[i],
      );
    });
    expect(filas[0].flujo_acumulado_cents).toBe(2500000); // C$ 25 000 (mes 1)
    expect(filas[1].flujo_acumulado_cents).toBe(5945000); // C$ 59 450 (mes 2)
  });

  it('totales del ano 1: GMV C$ 9,42 M, ingresos C$ 1 129 950, flujo +C$ 1 099 950', () => {
    const { totales } = servicio.calcular(supuestosDoc);
    expect(totales.gmv_cents).toBe(941625000);
    expect(totales.ingresos_cents).toBe(112995000);
    expect(totales.costos_cents).toBe(3000000);
    expect(totales.flujo_cents).toBe(109995000);
  });

  it('payback en el mes 2 (tabla 8.15): acumulado m2 C$ 59 450 >= inversion C$ 58 500', () => {
    const { filas } = servicio.calcular(supuestosDoc);
    const indicadores = servicio.indicadores(filas, supuestosDoc);
    expect(indicadores.payback_mes).toBe(2);
  });

  it('VAN positivo y TIR > 100 % (no estable)', () => {
    const { filas } = servicio.calcular(supuestosDoc);
    const indicadores = servicio.indicadores(filas, supuestosDoc);
    expect(indicadores.van_cents).toBeGreaterThan(0);
    expect(indicadores.tir_no_estable).toBe(true);
    expect(indicadores.margen_neto).toBeCloseTo(109995000 / 112995000, 4); // 97,3 %
  });

  it('punto de equilibrio de caja: GMV C$ 25 000 (~56 pedidos) (cap. 8.7)', () => {
    const { filas } = servicio.calcular(supuestosDoc);
    const indicadores = servicio.indicadores(filas, supuestosDoc);
    expect(indicadores.punto_equilibrio_caja_cents).toBe(2500000);
    expect(indicadores.pedidos_equilibrio_caja).toBe(56);
  });

  it('punto de equilibrio con costos variables (tabla 8.12: bajo/medio/alto)', () => {
    const { filas } = servicio.calcular(supuestosDoc);
    const indicadores = servicio.indicadores(filas, supuestosDoc);
    const porEscenario = Object.fromEntries(
      indicadores.escenarios_equilibrio.map((e) => [e.escenario, e]),
    );
    expect(porEscenario.bajo.pedidos_equilibrio).toBe(89); // C$ 40 050
    expect(porEscenario.bajo.gmv_equilibrio_cents).toBe(4005000);
    expect(porEscenario.medio.pedidos_equilibrio).toBe(167); // C$ 75 150
    expect(porEscenario.medio.gmv_equilibrio_cents).toBe(7515000);
    expect(porEscenario.alto.pedidos_equilibrio).toBe(1500); // C$ 675 000
    expect(porEscenario.alto.gmv_equilibrio_cents).toBe(67500000);
  });

  it('margen de seguridad frente al equilibrio medio (tabla 8.13)', () => {
    const { filas } = servicio.calcular(supuestosDoc);
    const indicadores = servicio.indicadores(filas, supuestosDoc);
    const porMes = Object.fromEntries(
      indicadores.margen_seguridad_por_mes.map((m) => [m.mes, m.margen_seguridad]),
    );
    expect(porMes[1]).toBeCloseTo(1 - 7515000 / 22500000, 4); // 66,6 %
    expect(porMes[3]).toBeCloseTo(1 - 7515000 / 39150000, 4); // 80,8 %
    expect(porMes[6]).toBeCloseTo(1 - 7515000 / 70875000, 4); // 89,4 %
    expect(porMes[12]).toBeCloseTo(1 - 7515000 / 141750000, 4); // 94,7 %
  });

  it('un churn del 5 % reduce la rampa de vendedores (tabla 8.6)', () => {
    const { filas } = servicio.calcular({ ...supuestosDoc, churn_tasa: 0.05 });
    expect(filas[1].vendedores).toBe(24); // 20 - 1 (churn) + 5
    expect(filas[11].vendedores).toBeLessThan(75);
  });

  it('rechaza una rampa de pedidos incompleta', () => {
    expect(() => servicio.calcular({ ...supuestosDoc, pedidos_por_vendedor: [25, 27] })).toThrow(
      'La rampa de pedidos por vendedor debe tener 12 valores',
    );
  });
});