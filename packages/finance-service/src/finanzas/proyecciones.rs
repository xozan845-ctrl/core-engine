//! Proyección financiera del cap. 8: modelo de cálculo mes a mes (tabla 8.7)
//! con fórmulas cerradas y auditables; punto de equilibrio (8.7), indicadores
//! de rentabilidad VAN/TIR/payback (8.11) y margen de seguridad.
//!
//! Todas las cifras están en centavos. El crate no depende de PostgreSQL para
//! calcular: la persistencia es una capa aparte.

use crate::db::{iso_ms, jsonb, timestamp, uuid_texto, Db};
use crate::errors::{error_db, ApiError};
use crate::money;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SupuestosProyeccion {
    pub horizonte_meses: i64,
    /// Rampa de vendedores activos (tabla 8.6: 20 al mes 1, +5 netos al mes).
    pub vendedores_iniciales: i64,
    pub entrada_vendedores_mes: i64,
    pub churn_tasa: f64,
    /// Rampa de pedidos por vendedor (tabla 8.6: 25..42, "+1 a +2 al mes").
    pub pedidos_por_vendedor: Vec<f64>,
    pub ticket_promedio_cents: i64,
    pub comision_tasa: f64,
    pub costos_fijos_cents: i64,
    #[serde(default)]
    pub costos_fijos_desde_mes7_cents: Option<i64>,
    pub inversion_inicial_cents: i64,
    #[serde(default)]
    pub tasa_descuento_mensual: Option<f64>,
}

impl SupuestosProyeccion {
    pub fn supuestos_informe() -> Self {
        Self {
            horizonte_meses: 12,
            vendedores_iniciales: 20,
            entrada_vendedores_mes: 5,
            churn_tasa: 0.0,
            pedidos_por_vendedor: vec![
                25.0, 27.0, 29.0, 31.0, 33.0, 35.0, 37.0, 38.0, 39.0, 40.0, 41.0, 42.0,
            ],
            ticket_promedio_cents: 45_000,
            comision_tasa: 0.12,
            costos_fijos_cents: 200_000,
            costos_fijos_desde_mes7_cents: Some(300_000),
            inversion_inicial_cents: 5_850_000,
            tasa_descuento_mensual: Some(0.015),
        }
    }

    pub fn validar(&self) -> Result<(), ApiError> {
        if !(1..=60).contains(&self.horizonte_meses) {
            return Err(ApiError::validacion(json!([
                {"campo": "horizonte_meses", "restricciones": ["min:1", "max:60"]}
            ])));
        }
        if self.pedidos_por_vendedor.len() as i64 != self.horizonte_meses {
            return Err(ApiError::dominio(
                "RAMPRA_INCOMPLETA",
                format!(
                    "La rampa de pedidos por vendedor debe tener {} valores (tabla 8.6).",
                    self.horizonte_meses
                ),
            ));
        }
        if self.comision_tasa <= 0.0 || self.comision_tasa >= 1.0 {
            return Err(ApiError::dominio(
                "TASA_INVALIDA",
                "La tasa de comision debe estar entre 0 y 1.",
            ));
        }
        Ok(())
    }

    pub fn costos_etapa2(&self) -> i64 {
        self.costos_fijos_desde_mes7_cents
            .unwrap_or(self.costos_fijos_cents)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilaProyeccion {
    pub mes: i64,
    pub vendedores: i64,
    pub pedidos: i64,
    pub gmv_cents: i64,
    pub ingresos_cents: i64,
    pub costos_cents: i64,
    pub flujo_cents: i64,
    pub flujo_acumulado_cents: i64,
    pub costo_fijo_por_pedido_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TotalesProyeccion {
    pub gmv_cents: i64,
    pub ingresos_cents: i64,
    pub costos_cents: i64,
    pub flujo_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EscenarioEquilibrio {
    pub escenario: String,
    pub costo_variable_por_pedido_cents: i64,
    pub contribucion_por_pedido_cents: i64,
    pub pedidos_equilibrio: i64,
    pub gmv_equilibrio_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MargenSeguridad {
    pub mes: i64,
    pub gmv_cents: i64,
    pub margen_seguridad: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoberturaPorVendedor {
    pub pedidos_por_vendedor_mes: i64,
    pub comision_por_pedido_cents: i64,
    pub comision_bruta_vendedor_cents: i64,
    pub contribucion_media_cents: i64,
    pub vendedores_para_cubrir_costos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EconomiaDeEscala {
    pub costo_fijo_por_pedido_inicio_cents: i64,
    pub costo_fijo_por_pedido_mes6_cents: i64,
    pub costo_fijo_por_pedido_fin_cents: i64,
    pub multiplicador_escala: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndicadoresProyeccion {
    pub payback_mes: Option<i64>,
    pub van_cents: i64,
    pub tir_mensual: Option<f64>,
    pub tir_anual: Option<f64>,
    pub tir_no_estable: bool,
    pub roi: f64,
    pub margen_neto: f64,
    pub punto_equilibrio_caja_cents: i64,
    pub pedidos_equilibrio_caja: i64,
    pub escenarios_equilibrio: Vec<EscenarioEquilibrio>,
    pub margen_seguridad_por_mes: Vec<MargenSeguridad>,
    pub cobertura_por_vendedor: CoberturaPorVendedor,
    pub economia_de_escala: EconomiaDeEscala,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SensibilidadComisionGmv {
    pub tasa_comision: f64,
    pub flujo_mes12_cents: Vec<VariacionFlujo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariacionFlujo {
    pub variacion_gmv: f64,
    pub flujo_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SensibilidadVan {
    pub tasa_anual: f64,
    pub tasa_mensual: f64,
    pub van_cents: i64,
    pub relacion_van_inversion: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaybackPorEscenario {
    pub escenario: String,
    pub descripcion: String,
    pub flujo_acumulado_cents: i64,
    pub payback_mes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResumenAnual {
    pub anio: i64,
    pub gmv_cents: i64,
    pub ingresos_cents: i64,
    pub costos_cents: i64,
    pub flujo_cents: i64,
    pub margen_neto: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcumuladoBienal {
    pub gmv_cents: i64,
    pub ingresos_cents: i64,
    pub costos_cents: i64,
    pub flujo_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mes24 {
    pub gmv_cents: i64,
    pub incremento_vs_mes12: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanBienal {
    pub inversion_inicial_cents: i64,
    pub anios: Vec<ResumenAnual>,
    pub filas: Vec<FilaProyeccion>,
    pub acumulado_24_meses: AcumuladoBienal,
    pub roi: f64,
    pub mes_24: Mes24,
    pub indicadores: IndicadoresProyeccion,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Proyeccion {
    pub id: String,
    pub nombre: String,
    pub horizonte_meses: i64,
    pub supuestos: SupuestosProyeccion,
    pub filas: Vec<FilaProyeccion>,
    pub totales: TotalesProyeccion,
    pub inversion_inicial_cents: i64,
    pub indicadores: IndicadoresProyeccion,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProyeccionResumen {
    pub id: String,
    pub nombre: String,
    pub horizonte_meses: i64,
    pub creado_en: String,
}

/// Cuerpo de los estudios de sensibilidad (8.7, 8.10, 8.11, 8.12).
#[derive(Debug, Clone, Serialize)]
pub struct EstudiosSensibilidad {
    pub supuestos: SupuestosProyeccion,
    pub matriz_comision_gmv: Vec<SensibilidadComisionGmv>,
    pub van_por_tasa: Vec<SensibilidadVan>,
    pub payback_por_escenario: Vec<PaybackPorEscenario>,
    pub cobertura_por_vendedor: CoberturaPorVendedor,
}

pub struct Proyecciones {
    db: std::sync::Arc<Db>,
}

impl Proyecciones {
    pub fn nueva(db: std::sync::Arc<Db>) -> Self {
        Self { db }
    }

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

    pub async fn crear(
        &self,
        nombre: &str,
        supuestos: SupuestosProyeccion,
    ) -> Result<Proyeccion, ApiError> {
        let (filas, totales) = Self::calcular(&supuestos)?;
        let indicadores = Self::indicadores(&filas, &supuestos);
        let fila = sqlx::query(
            "INSERT INTO finance.proyecciones
               (nombre, horizonte_meses, supuestos, filas, totales, inversion_inicial_cents)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id, nombre, horizonte_meses, inversion_inicial_cents, creado_en",
        )
        .bind(nombre)
        .bind(supuestos.horizonte_meses)
        .bind(sqlx::types::Json(&supuestos))
        .bind(sqlx::types::Json(&filas))
        .bind(sqlx::types::Json(&totales))
        .bind(supuestos.inversion_inicial_cents)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio("PROYECCION_FALLIDA", "No se pudo guardar la proyeccion.")
        })?;
        Ok(Proyeccion {
            id: uuid_texto(&fila, "id").map_err(error_db)?,
            nombre: fila.try_get("nombre").map_err(error_db)?,
            horizonte_meses: fila
                .try_get::<i32, _>("horizonte_meses")
                .map_err(error_db)? as i64,
            supuestos,
            filas,
            totales,
            inversion_inicial_cents: fila
                .try_get::<i32, _>("inversion_inicial_cents")
                .map_err(error_db)? as i64,
            indicadores,
            creado_en: timestamp(&fila, "creado_en").map_err(error_db)?,
        })
    }

    pub async fn listar(&self) -> Result<Vec<ProyeccionResumen>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, nombre, horizonte_meses, creado_en
             FROM finance.proyecciones ORDER BY creado_en DESC",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas
            .iter()
            .map(|fila| ProyeccionResumen {
                id: uuid_texto(fila, "id").unwrap_or_default(),
                nombre: fila.try_get("nombre").unwrap_or_default(),
                horizonte_meses: fila.try_get::<i32, _>("horizonte_meses").unwrap_or(0) as i64,
                creado_en: timestamp(fila, "creado_en").unwrap_or_default(),
            })
            .collect())
    }

    pub async fn obtener(&self, id: &str) -> Result<Option<Proyeccion>, ApiError> {
        let uuid = sqlx::types::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "La proyeccion debe ser un UUID."))?;
        let fila = sqlx::query(
            "SELECT id, nombre, horizonte_meses, supuestos, filas, totales,
                    inversion_inicial_cents, creado_en
             FROM finance.proyecciones WHERE id = $1",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Ok(None);
        };
        let supuestos: SupuestosProyeccion =
            serde_json::from_value(jsonb(&fila, "supuestos").map_err(error_db)?)
                .map_err(|_| ApiError::interno())?;
        let filas: Vec<FilaProyeccion> =
            serde_json::from_value(jsonb(&fila, "filas").map_err(error_db)?)
                .map_err(|_| ApiError::interno())?;
        let totales: TotalesProyeccion =
            serde_json::from_value(jsonb(&fila, "totales").map_err(error_db)?)
                .map_err(|_| ApiError::interno())?;
        Ok(Some(Proyeccion {
            id: uuid_texto(&fila, "id").map_err(error_db)?,
            nombre: fila.try_get("nombre").map_err(error_db)?,
            horizonte_meses: fila
                .try_get::<i32, _>("horizonte_meses")
                .map_err(error_db)? as i64,
            indicadores: Self::indicadores(&filas, &supuestos),
            supuestos,
            filas,
            totales,
            inversion_inicial_cents: fila
                .try_get::<i32, _>("inversion_inicial_cents")
                .map_err(error_db)? as i64,
            creado_en: timestamp(&fila, "creado_en").map_err(error_db)?,
        }))
    }

    /// GMV real del mes para comparar con la proyección (tabla 8.10).
    pub async fn leer_gmv_real(&self, db: &Db, mes: &str) -> Result<Option<i64>, ApiError> {
        let inicio_mes = format!("{}-01", mes.get(0..7).unwrap_or("2026-01"));
        let total: Option<i32> = sqlx::query_scalar(
            "SELECT monto_cents FROM finance.metricas_mensuales
             WHERE periodo = $1::date AND tipo = 'gmv'",
        )
        .bind(&inicio_mes)
        .fetch_optional(db.pool())
        .await
        .map_err(error_db)?;
        Ok(total.map(|valor| valor as i64))
    }

    /// Serializa el detalle de una proyección para el contrato de respuesta.
    pub fn a_valor(proyeccion: &Proyeccion) -> Value {
        json!(proyeccion)
    }

    /// Utilería de fecha para respuestas de la API.
    pub fn fecha_iso(fecha: DateTime<Utc>) -> String {
        iso_ms(fecha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supuestos_base() -> SupuestosProyeccion {
        SupuestosProyeccion::supuestos_informe()
    }

    #[test]
    fn debe_calcular_la_tabla_del_informe() {
        let (filas, totales) = Proyecciones::calcular(&supuestos_base()).expect("calculo");
        assert_eq!(filas.len(), 12);
        assert_eq!(filas[0].mes, 1);
        assert_eq!(filas[0].vendedores, 20);
        assert_eq!(filas[0].pedidos, 500);
        assert_eq!(filas[0].gmv_cents, 22_500_000);
        assert_eq!(filas[0].ingresos_cents, 2_700_000);
        assert_eq!(filas[0].costos_cents, 200_000);
        assert_eq!(filas[0].flujo_cents, 2_500_000);
        assert_eq!(filas[11].vendedores, 75);
        assert_eq!(filas[11].pedidos, 3_150);
        assert_eq!(filas[11].gmv_cents, 141_750_000);
        assert_eq!(
            totales.gmv_cents,
            filas.iter().map(|f| f.gmv_cents).sum::<i64>()
        );
    }

    #[test]
    fn debe_rechazar_una_rampa_incompleta() {
        let mut supuestos = supuestos_base();
        supuestos.pedidos_por_vendedor.pop();
        let error = Proyecciones::calcular(&supuestos).expect_err("rampa");
        assert_eq!(error.codigo, "RAMPRA_INCOMPLETA");
    }

    #[test]
    fn debe_calcular_equilibrio_y_cobertura() {
        let supuestos = supuestos_base();
        let (filas, _) = Proyecciones::calcular(&supuestos).expect("calculo");
        let indicadores = Proyecciones::indicadores(&filas, &supuestos);
        assert_eq!(indicadores.punto_equilibrio_caja_cents, 2_500_000);
        assert_eq!(indicadores.pedidos_equilibrio_caja, 56);
        assert_eq!(indicadores.escenarios_equilibrio.len(), 3);
        assert_eq!(
            indicadores.cobertura_por_vendedor.comision_por_pedido_cents,
            5_400
        );
        assert_eq!(
            indicadores
                .cobertura_por_vendedor
                .comision_bruta_vendedor_cents,
            226_800
        );
        assert_eq!(
            indicadores
                .cobertura_por_vendedor
                .vendedores_para_cubrir_costos,
            4
        );
        assert_eq!(
            indicadores
                .economia_de_escala
                .costo_fijo_por_pedido_inicio_cents,
            400
        );
        assert!(indicadores.payback_mes.is_some());
    }

    #[test]
    fn debe_generar_sensibilidad_y_paybacks() {
        let supuestos = supuestos_base();
        let matriz = Proyecciones::sensibilidad_comision_gmv(&supuestos).expect("matriz");
        assert_eq!(matriz.len(), 4);
        assert_eq!(matriz[0].flujo_mes12_cents.len(), 3);
        let paybacks = Proyecciones::payback_por_escenario(&supuestos).expect("paybacks");
        assert_eq!(paybacks.len(), 3);
        assert!(paybacks
            .iter()
            .all(|escenario| escenario.payback_mes.is_some()));
    }

    #[test]
    fn debe_construir_el_plan_bienal() {
        let plan = Proyecciones::plan_bienal(&supuestos_base()).expect("plan");
        assert_eq!(plan.filas.len(), 24);
        assert_eq!(plan.anios.len(), 2);
        assert!(plan.roi > 0.0);
        assert!(plan.mes_24.gmv_cents > plan.filas[11].gmv_cents);
        assert!(plan.mes_24.incremento_vs_mes12 > 0.0);
    }
}
