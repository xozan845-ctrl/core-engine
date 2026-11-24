use super::prelude::*;
use super::*;

impl Proyecciones {
    /// Modelo cerrado: vendedores -> pedidos -> GMV -> ingresos -> flujo.
    pub fn calcular(
        supuestos: &SupuestosProyeccion,
    ) -> Result<(Vec<FilaProyeccion>, TotalesProyeccion), ApiError> {
        supuestos.validar()?;
        let n = supuestos.horizonte_meses as usize;
        let mut filas = Vec::with_capacity(n);
        let mut vendedores = supuestos.vendedores_iniciales;
        let mut acumulado = 0i64;
        let costos_etapa2 = supuestos.costos_etapa2();

        for t in 1..=n {
            if t > 1 {
                let churn = ((vendedores as f64) * supuestos.churn_tasa).round() as i64;
                vendedores = vendedores - churn + supuestos.entrada_vendedores_mes;
            }
            let pedidos =
                ((vendedores as f64) * supuestos.pedidos_por_vendedor[t - 1]).round() as i64;
            let gmv = pedidos * supuestos.ticket_promedio_cents;
            let ingresos = (gmv as f64 * supuestos.comision_tasa).round() as i64;
            let costos = if t <= 6 {
                supuestos.costos_fijos_cents
            } else {
                costos_etapa2
            };
            let flujo = ingresos - costos;
            let fila = FilaProyeccion {
                mes: t as i64,
                vendedores,
                pedidos,
                gmv_cents: gmv,
                ingresos_cents: ingresos,
                costos_cents: costos,
                flujo_cents: flujo,
                flujo_acumulado_cents: acumulado + flujo,
                costo_fijo_por_pedido_cents: if pedidos > 0 {
                    (costos as f64 / pedidos as f64).round() as i64
                } else {
                    0
                },
            };
            acumulado = fila.flujo_acumulado_cents;
            filas.push(fila);
        }

        let totales = TotalesProyeccion {
            gmv_cents: filas.iter().map(|fila| fila.gmv_cents).sum(),
            ingresos_cents: filas.iter().map(|fila| fila.ingresos_cents).sum(),
            costos_cents: filas.iter().map(|fila| fila.costos_cents).sum(),
            flujo_cents: filas.iter().map(|fila| fila.flujo_cents).sum(),
        };
        Ok((filas, totales))
    }

    /// Indicadores de rentabilidad (8.11) y equilibrio (8.7) sobre las filas.
    pub fn indicadores(
        filas: &[FilaProyeccion],
        supuestos: &SupuestosProyeccion,
    ) -> IndicadoresProyeccion {
        let inversion = supuestos.inversion_inicial_cents;
        let tasa_mensual = supuestos.tasa_descuento_mensual.unwrap_or(0.015);
        let ticket = supuestos.ticket_promedio_cents;
        let tasa = supuestos.comision_tasa;
        let costos_fijos = if filas.len() >= 7 {
            supuestos.costos_etapa2()
        } else {
            supuestos.costos_fijos_cents
        };

        let payback = filas
            .iter()
            .find(|fila| fila.flujo_acumulado_cents >= inversion)
            .map(|fila| fila.mes);

        let van = filas
            .iter()
            .enumerate()
            .map(|(indice, fila)| {
                fila.flujo_cents as f64 / (1.0 + tasa_mensual).powi(indice as i32 + 1)
            })
            .sum::<f64>()
            - inversion as f64;

        let tir = Self::tir_mensual(filas, inversion);
        let tir_anual = tir.map(|tir| ((1.0 + tir).powi(12) - 1.0) * 100.0);

        let equilibrio_caja = costos_fijos as f64 / tasa;
        let pedidos_equilibrio = ((equilibrio_caja / ticket as f64).ceil() as i64).max(1);

        let ingreso_por_pedido = money::multiplicar_por(ticket, tasa)
            .unwrap_or_else(|_| (ticket as f64 * tasa).round() as i64);
        let escenarios = [("bajo", 2_000i64), ("medio", 3_600), ("alto", 5_200)];
        let escenarios_equilibrio: Vec<EscenarioEquilibrio> = escenarios
            .iter()
            .map(|(escenario, costo_variable)| {
                let contribucion = ingreso_por_pedido - costo_variable;
                let pedidos = if contribucion > 0 {
                    (costos_fijos as f64 / contribucion as f64).ceil() as i64
                } else {
                    0
                };
                EscenarioEquilibrio {
                    escenario: escenario.to_string(),
                    costo_variable_por_pedido_cents: *costo_variable,
                    contribucion_por_pedido_cents: contribucion,
                    pedidos_equilibrio: pedidos,
                    gmv_equilibrio_cents: pedidos * ticket,
                }
            })
            .collect();

        let equilibrio_medio = escenarios_equilibrio
            .iter()
            .find(|escenario| escenario.escenario == "medio")
            .map(|escenario| escenario.gmv_equilibrio_cents)
            .unwrap_or(0);
        let margen_seguridad_por_mes = filas
            .iter()
            .filter(|fila| [1, 3, 6, 12].contains(&fila.mes))
            .map(|fila| MargenSeguridad {
                mes: fila.mes,
                gmv_cents: fila.gmv_cents,
                margen_seguridad: if equilibrio_medio > 0 && fila.gmv_cents > 0 {
                    1.0 - equilibrio_medio as f64 / fila.gmv_cents as f64
                } else {
                    0.0
                },
            })
            .collect();

        let total_flujo: i64 = filas.iter().map(|fila| fila.flujo_cents).sum();
        let total_ingresos: i64 = filas.iter().map(|fila| fila.ingresos_cents).sum();

        IndicadoresProyeccion {
            payback_mes: payback,
            van_cents: van.round() as i64,
            tir_mensual: tir,
            tir_anual: tir_anual.map(|valor| (valor * 100.0).round() / 100.0),
            tir_no_estable: tir_anual.map(|valor| valor > 100.0).unwrap_or(true),
            roi: if inversion > 0 {
                total_flujo as f64 / inversion as f64
            } else {
                0.0
            },
            margen_neto: if total_ingresos > 0 {
                total_flujo as f64 / total_ingresos as f64
            } else {
                0.0
            },
            punto_equilibrio_caja_cents: equilibrio_caja.round() as i64,
            pedidos_equilibrio_caja: pedidos_equilibrio,
            escenarios_equilibrio,
            margen_seguridad_por_mes,
            cobertura_por_vendedor: Self::cobertura_por_vendedor(supuestos),
            economia_de_escala: Self::economia_de_escala(filas),
        }
    }

    /// Cobertura por vendedor (8.7): comisión bruta vs. costo fijo.
    pub fn cobertura_por_vendedor(supuestos: &SupuestosProyeccion) -> CoberturaPorVendedor {
        let pedidos_por_vendedor = supuestos
            .pedidos_por_vendedor
            .last()
            .copied()
            .unwrap_or(25.0)
            .round() as i64;
        let comision_por_pedido =
            money::multiplicar_por(supuestos.ticket_promedio_cents, supuestos.comision_tasa)
                .unwrap_or(0);
        let contribucion_media = 1_800;
        let costos_fijos = if supuestos.pedidos_por_vendedor.len() >= 7 {
            supuestos.costos_etapa2()
        } else {
            supuestos.costos_fijos_cents
        };
        let vendedores = if contribucion_media * pedidos_por_vendedor > 0 {
            (costos_fijos as f64 / (contribucion_media * pedidos_por_vendedor) as f64).ceil() as i64
        } else {
            1
        };
        CoberturaPorVendedor {
            pedidos_por_vendedor_mes: pedidos_por_vendedor,
            comision_por_pedido_cents: comision_por_pedido,
            comision_bruta_vendedor_cents: comision_por_pedido * pedidos_por_vendedor,
            contribucion_media_cents: contribucion_media,
            vendedores_para_cubrir_costos: vendedores.max(1),
        }
    }

    /// Economía de escala (8.7): costo fijo de plataforma por pedido.
    pub fn economia_de_escala(filas: &[FilaProyeccion]) -> EconomiaDeEscala {
        let inicio = filas
            .first()
            .map(|f| f.costo_fijo_por_pedido_cents)
            .unwrap_or(0);
        let mes6 = filas
            .iter()
            .find(|fila| fila.mes == 6)
            .map(|fila| fila.costo_fijo_por_pedido_cents)
            .unwrap_or(0);
        let fin = filas
            .iter()
            .find(|fila| fila.mes == 12)
            .or_else(|| filas.last())
            .map(|fila| fila.costo_fijo_por_pedido_cents)
            .unwrap_or(0);
        EconomiaDeEscala {
            costo_fijo_por_pedido_inicio_cents: inicio,
            costo_fijo_por_pedido_mes6_cents: mes6,
            costo_fijo_por_pedido_fin_cents: fin,
            multiplicador_escala: if fin > 0 {
                ((inicio as f64 / fin as f64) * 100.0).round() / 100.0
            } else {
                0.0
            },
        }
    }

    /// Matriz de sensibilidad comisión x demanda (8.10, tabla 8.12).
    pub fn sensibilidad_comision_gmv(
        supuestos: &SupuestosProyeccion,
    ) -> Result<Vec<SensibilidadComisionGmv>, ApiError> {
        let tasas = [0.08, 0.1, 0.12, 0.14];
        let variaciones = [-0.3, 0.0, 0.15];
        let ultimo_mes = supuestos.pedidos_por_vendedor.len();
        let mut resultado = Vec::new();
        for tasa in tasas {
            let mut flujos = Vec::new();
            for variacion in variaciones {
                let modificados = SupuestosProyeccion {
                    comision_tasa: tasa,
                    pedidos_por_vendedor: supuestos
                        .pedidos_por_vendedor
                        .iter()
                        .map(|pedidos| pedidos * (1.0 + variacion))
                        .collect(),
                    ..supuestos.clone()
                };
                let (filas, _) = Self::calcular(&modificados)?;
                flujos.push(VariacionFlujo {
                    variacion_gmv: variacion,
                    flujo_cents: filas[ultimo_mes - 1].flujo_cents,
                });
            }
            resultado.push(SensibilidadComisionGmv {
                tasa_comision: tasa,
                flujo_mes12_cents: flujos,
            });
        }
        Ok(resultado)
    }

    /// Sensibilidad del VAN al costo de oportunidad (8.11, tabla 8.12).
    pub fn sensibilidad_van(
        filas: &[FilaProyeccion],
        supuestos: &SupuestosProyeccion,
    ) -> Vec<SensibilidadVan> {
        let inversion = supuestos.inversion_inicial_cents;
        [0.12f64, 0.18, 0.24, 0.3]
            .iter()
            .map(|tasa_anual| {
                let tasa_mensual = tasa_anual / 12.0;
                let van = filas
                    .iter()
                    .enumerate()
                    .map(|(indice, fila)| {
                        fila.flujo_cents as f64 / (1.0 + tasa_mensual).powi(indice as i32 + 1)
                    })
                    .sum::<f64>()
                    - inversion as f64;
                SensibilidadVan {
                    tasa_anual: *tasa_anual,
                    tasa_mensual,
                    van_cents: van.round() as i64,
                    relacion_van_inversion: if inversion > 0 {
                        ((van / inversion as f64) * 10.0).round() / 10.0
                    } else {
                        0.0
                    },
                }
            })
            .collect()
    }

    /// Payback por escenario (8.11, tabla 8.12).
    pub fn payback_por_escenario(
        supuestos: &SupuestosProyeccion,
    ) -> Result<Vec<PaybackPorEscenario>, ApiError> {
        let con_escenario = |escenario: &str,
                             factor_pedidos: f64,
                             tasa: f64|
         -> Result<PaybackPorEscenario, ApiError> {
            let modificados = SupuestosProyeccion {
                comision_tasa: tasa,
                pedidos_por_vendedor: supuestos
                    .pedidos_por_vendedor
                    .iter()
                    .map(|pedidos| pedidos * factor_pedidos)
                    .collect(),
                ..supuestos.clone()
            };
            let (filas, _) = Self::calcular(&modificados)?;
            let inversion = supuestos.inversion_inicial_cents;
            let payback = filas
                .iter()
                .find(|fila| fila.flujo_acumulado_cents >= inversion)
                .map(|fila| fila.mes);
            let acumulado = payback
                .and_then(|mes| filas.get(mes as usize - 1))
                .map(|fila| fila.flujo_acumulado_cents)
                .unwrap_or(0);
            let descripcion = match escenario {
                "base" => "Recupera la inversion en el segundo mes.",
                "pesimista" => "Aun con 30 % menos de pedidos, caja positiva continua.",
                _ => "Con la comision minima y demanda baja, se recupera en el mes 4.",
            };
            Ok(PaybackPorEscenario {
                escenario: escenario.to_string(),
                descripcion: descripcion.to_string(),
                flujo_acumulado_cents: acumulado,
                payback_mes: payback,
            })
        };
        Ok(vec![
            con_escenario("base", 1.0, supuestos.comision_tasa)?,
            con_escenario("pesimista", 0.7, supuestos.comision_tasa)?,
            con_escenario("peor_combinado", 0.7, 0.08)?,
        ])
    }

    /// Supuestos del segundo año (8.9).
    pub fn preset_segundo_anio() -> SupuestosProyeccion {
        SupuestosProyeccion {
            horizonte_meses: 12,
            vendedores_iniciales: 78,
            entrada_vendedores_mes: 3,
            churn_tasa: 0.0,
            pedidos_por_vendedor: vec![
                42.0, 42.0, 42.0, 42.0, 42.0, 42.0, 43.0, 43.0, 43.0, 43.0, 43.0, 43.0,
            ],
            ticket_promedio_cents: 45_000,
            comision_tasa: 0.12,
            costos_fijos_cents: 300_000,
            costos_fijos_desde_mes7_cents: Some(300_000),
            inversion_inicial_cents: 5_850_000,
            tasa_descuento_mensual: Some(0.015),
        }
    }

    /// Plan a 24 meses: año 1 (tabla 8.6) + año 2 indicativo (8.9).
    pub fn plan_bienal(supuestos_anio1: &SupuestosProyeccion) -> Result<PlanBienal, ApiError> {
        let (filas_anio1, _) = Self::calcular(supuestos_anio1)?;
        let (filas_anio2, _) = Self::calcular(&Self::preset_segundo_anio())?;
        let filas: Vec<FilaProyeccion> = filas_anio1
            .iter()
            .chain(filas_anio2.iter())
            .cloned()
            .collect();
        let inversion = supuestos_anio1.inversion_inicial_cents;
        let resumen = |anio: i64, filas: &[FilaProyeccion]| ResumenAnual {
            anio,
            gmv_cents: filas.iter().map(|fila| fila.gmv_cents).sum(),
            ingresos_cents: filas.iter().map(|fila| fila.ingresos_cents).sum(),
            costos_cents: filas.iter().map(|fila| fila.costos_cents).sum(),
            flujo_cents: filas.iter().map(|fila| fila.flujo_cents).sum(),
            margen_neto: {
                let ingresos: i64 = filas.iter().map(|fila| fila.ingresos_cents).sum();
                let flujo: i64 = filas.iter().map(|fila| fila.flujo_cents).sum();
                if ingresos > 0 {
                    flujo as f64 / ingresos as f64
                } else {
                    0.0
                }
            },
        };
        let anios = vec![resumen(1, &filas_anio1), resumen(2, &filas_anio2)];
        let flujo_total: i64 = anios.iter().map(|anio| anio.flujo_cents).sum();
        let mes24 = filas.last().cloned().ok_or_else(ApiError::interno)?;
        let supuestos_24 = SupuestosProyeccion {
            horizonte_meses: 24,
            ..supuestos_anio1.clone()
        };
        let indicadores = Self::indicadores(&filas, &supuestos_24);
        Ok(PlanBienal {
            inversion_inicial_cents: inversion,
            anios: anios.clone(),
            filas,
            acumulado_24_meses: AcumuladoBienal {
                gmv_cents: anios.iter().map(|anio| anio.gmv_cents).sum(),
                ingresos_cents: anios.iter().map(|anio| anio.ingresos_cents).sum(),
                costos_cents: anios.iter().map(|anio| anio.costos_cents).sum(),
                flujo_cents: flujo_total,
            },
            roi: if inversion > 0 {
                flujo_total as f64 / inversion as f64
            } else {
                0.0
            },
            mes_24: Mes24 {
                gmv_cents: mes24.gmv_cents,
                incremento_vs_mes12: if filas_anio1[11].gmv_cents > 0 {
                    mes24.gmv_cents as f64 / filas_anio1[11].gmv_cents as f64 - 1.0
                } else {
                    0.0
                },
            },
            indicadores,
        })
    }

    /// TIR mensual por búsqueda binaria en [0, 5] (500 %/mes máx.).
    fn tir_mensual(filas: &[FilaProyeccion], inversion: i64) -> Option<f64> {
        let f = |r: f64| {
            filas
                .iter()
                .enumerate()
                .map(|(indice, fila)| fila.flujo_cents as f64 / (1.0 + r).powi(indice as i32 + 1))
                .sum::<f64>()
                - inversion as f64
        };
        if f(0.0) <= 0.0 {
            return Some(0.0);
        }
        if f(5.0) > 0.0 {
            return None;
        }
        let (mut bajo, mut alto) = (0.0, 5.0);
        for _ in 0..200 {
            let medio = (bajo + alto) / 2.0;
            if f(medio) > 0.0 {
                bajo = medio;
            } else {
                alto = medio;
            }
        }
        Some((bajo + alto) / 2.0)
    }

    pub fn estudios(supuestos: &SupuestosProyeccion) -> Result<EstudiosSensibilidad, ApiError> {
        let (filas, _) = Self::calcular(supuestos)?;
        Ok(EstudiosSensibilidad {
            supuestos: supuestos.clone(),
            matriz_comision_gmv: Self::sensibilidad_comision_gmv(supuestos)?,
            van_por_tasa: Self::sensibilidad_van(&filas, supuestos),
            payback_por_escenario: Self::payback_por_escenario(supuestos)?,
            cobertura_por_vendedor: Self::cobertura_por_vendedor(supuestos),
        })
    }
}
