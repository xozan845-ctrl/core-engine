import { ProyeccionesService, SupuestosProyeccion } from './proyecciones.service';

/**
 * Estudios del cap. 8 que el informe exige (tablas 8.12-8.15): matriz de
 * sensibilidad comision x demanda, VAN por tasa de descuento, payback por
 * escenario, cobertura por vendedor y economia de escala; plan a 24 meses
 * (8.9) con cuenta de resultados por anio y ROI ~= 63x.
 */
describe('ProyeccionesService (estudios del cap. 8)', () => {
  const servicio = new ProyeccionesService({} as never);

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
    inversion_inicial_cents: 5850000, // C$ 58 500
    tasa_descuento_mensual: 0.015, // 18 % anual / 12
  };

  it('matriz de sensibilidad (8.10, tabla 8.12): fila 12 % -> 116 070 / 167 100 / 192 615', () => {
    const matriz = servicio.sensibilidadComisionGmv(supuestosDoc);
    expect(matriz).toHaveLength(4);
    const fila = (tasa: number) => Object.fromEntries(
      matriz.find((m) => m.tasa_comision === tasa)!.flujo_mes12_cents.map((f) => [f.variacion_gmv, f.flujo_cents]),
    );
    // 12 %
    expect(fila(0.12)[-0.3]).toBe(11607000); // C$ 116 070
    expect(fila(0.12)[0]).toBe(16710000); // C$ 167 100
    expect(fila(0.12)[0.15]).toBe(19261500); // C$ 192 615
    // 8 %
    expect(fila(0.08)[0]).toBe(11040000); // C$ 110 400
    // 10 %
    expect(fila(0.10)[-0.3]).toBe(9622500); // C$ 96 225
    // 14 %
    expect(fila(0.14)[0.15]).toBe(22521750); // C$ 225 217,50
    // mensualidad: todas las celdas mantienen flujo positivo
    matriz.forEach((m) =>
      m.flujo_mes12_cents.forEach((f) => expect(f.flujo_cents).toBeGreaterThan(0)),
    );
  });

  it('VAN por tasa de descuento (8.11, tabla 8.12): 91,6 M @18 % y monotonia decreciente', () => {
    const { filas } = servicio.calcular(supuestosDoc);
    const vanes = servicio.sensibilidadVan(filas, supuestosDoc);
    const porTasa = Object.fromEntries(vanes.map((v) => [v.tasa_anual, v.van_cents]));
    expect(porTasa[0.18]).toBe(91606321); // C$ 916 063,21 @18 %
    expect(porTasa[0.12]).toBeGreaterThan(porTasa[0.18]);
    expect(porTasa[0.18]).toBeGreaterThan(porTasa[0.24]);
    expect(porTasa[0.24]).toBeGreaterThan(porTasa[0.3]);
    expect(porTasa[0.3]).toBe(84244286); // C$ 842 442,86 @30 %
    expect(vanes.find((v) => v.tasa_anual === 0.12)?.relacion_van_inversion).toBe(16.3);
  });

  it('payback por escenario (8.11, tabla 8.12): mes 2 base, mes 3 pesimista, mes 4 peor', () => {
    const escenarios = servicio.paybackPorEscenario(supuestosDoc);
    const porEscenario = Object.fromEntries(escenarios.map((e) => [e.escenario, e]));
    expect(porEscenario.base.payback_mes).toBe(2);
    expect(porEscenario.base.flujo_acumulado_cents).toBe(5945000); // C$ 59 450
    expect(porEscenario.pesimista.payback_mes).toBe(3);
    expect(porEscenario.pesimista.flujo_acumulado_cents).toBe(7130100); // C$ 71 301
    expect(porEscenario.peor_combinado.payback_mes).toBe(4);
    expect(porEscenario.peor_combinado.flujo_acumulado_cents).toBe(7087600); // C$ 70 876
  });

  it('cobertura por vendedor (8.7): C$ 54/pedido, C$ 2 268 brutos y ~4 vendedores', () => {
    const cobertura = servicio.coberturaPorVendedor(supuestosDoc);
    expect(cobertura.comision_por_pedido_cents).toBe(5400); // C$ 54
    expect(cobertura.comision_bruta_vendedor_cents).toBe(226800); // C$ 2 268
    expect(cobertura.vendedores_para_cubrir_costos).toBe(4);
  });

  it('economia de escala (8.7): costo fijo por pedido C$ 4,00 -> C$ 0,95 (4,21x)', () => {
    const { filas } = servicio.calcular(supuestosDoc);
    const economia = servicio.economiaDeEscala(filas);
    expect(economia.costo_fijo_por_pedido_inicio_cents).toBe(400); // C$ 4,00 (m1)
    expect(economia.costo_fijo_por_pedido_mes6_cents).toBe(127); // C$ 1,27 (m6)
    expect(economia.costo_fijo_por_pedido_fin_cents).toBe(95); // C$ 0,95 (m12)
    expect(economia.multiplicador_escala).toBe(4.21);
  });

  it('preset del segundo anio (8.9): 48 249 pedidos y GMV C$ 21 712 050', () => {
    const anio2 = servicio.calcular(servicio.presetSegundoAnio());
    expect(anio2.totales.gmv_cents).toBe(2171205000); // C$ 21 712 050
    expect(anio2.totales.ingresos_cents).toBe(260544600); // C$ 2 605 446
    expect(anio2.totales.costos_cents).toBe(3600000); // C$ 36 000
    expect(anio2.totales.flujo_cents).toBe(256944600); // C$ 2 569 446
    const pedidos = anio2.filas.reduce((s, f) => s + f.pedidos, 0);
    expect(pedidos).toBe(48249);
  });

  it('plan a 24 meses (8.9): cuenta por anio, ROI 62,7x y mes 24 +52 % sobre el mes 12', () => {
    const plan = servicio.planBienal(supuestosDoc);
    expect(plan.anios).toHaveLength(2);
    // tabla 8.11: anio 1
    expect(plan.anios[0].gmv_cents).toBe(941625000); // C$ 9 416 250
    expect(plan.anios[0].ingresos_cents).toBe(112995000); // C$ 1 129 950
    expect(plan.anios[0].flujo_cents).toBe(109995000); // +C$ 1 099 950
    expect(plan.anios[0].margen_neto).toBeCloseTo(0.9735, 3); // 97,3 %
    // anio 2
    expect(plan.anios[1].gmv_cents).toBe(2171205000); // C$ 21 712 050
    expect(plan.anios[1].flujo_cents).toBe(256944600); // +C$ 2 569 446
    expect(plan.anios[1].margen_neto).toBeCloseTo(0.9862, 3); // 98,6 %
    // cierre del mes 24 (111 vendedores x 43 pedidos x C$ 450)
    expect(plan.filas).toHaveLength(24);
    expect(plan.filas[23].vendedores).toBe(111);
    expect(plan.filas[23].gmv_cents).toBe(214785000); // C$ 2 147 850
    expect(plan.filas[23].gmv_cents / plan.filas[11].gmv_cents).toBeCloseTo(1.5152, 3); // +52 %
    // ROI acumulado: C$ 3 669 396 / C$ 58 500 = 62,7x (doc: ~63x)
    const flujoTotal = plan.anios[0].flujo_cents + plan.anios[1].flujo_cents;
    expect(flujoTotal / supuestosDoc.inversion_inicial_cents).toBeCloseTo(62.7, 1);
    expect(flujoTotal).toBe(366939600); // C$ 3 669 396
  });

  it('indicadores del plan a 24 meses: payback 2, VAN positivo y 63x de retorno', () => {
    const plan = servicio.planBienal(supuestosDoc);
    const indicadores = servicio.indicadores(plan.filas, supuestosDoc);
    expect(indicadores.payback_mes).toBe(2);
    expect(indicadores.van_cents).toBeGreaterThan(0);
    expect(indicadores.tir_no_estable).toBe(true);
    expect(indicadores.roi).toBeCloseTo(62.7, 1);
  });
});